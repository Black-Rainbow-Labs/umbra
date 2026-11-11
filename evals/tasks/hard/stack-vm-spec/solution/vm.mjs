// Interpreter for the SVM stack language (see SPEC.md).

import fs from "node:fs";

const MIN = -(2n ** 63n);
const MAX = 2n ** 63n - 1n;
const STEP_LIMIT = 1000000;
const CALL_LIMIT = 256;
const OPERANDS = { PUSH: 1, POP: 0, DUP: 0, SWAP: 0, OVER: 0, ADD: 0, SUB: 0, MUL: 0, DIV: 0, MOD: 0, LT: 0, EQ: 0, JMP: 1, JZ: 1, CALL: 1, RET: 0, PRINT: 0, HALT: 0 };
const LABEL_LINE = /^([A-Za-z_][A-Za-z0-9_]*):$/;
const INTEGER = /^-?[0-9]+$/;

/** An error with the line number it is reported against. */
class VmError extends Error {
  constructor(line, message) {
    super(message);
    this.line = line;
  }
}

/**
 * Splits program text into instructions and labels, collecting every error found before running.
 * @param text - the program text
 */
function parseProgram(text) {
  const instructions = [];
  const labels = new Map();
  const errors = [];
  text.split("\n").forEach((raw, index) => {
    const line = index + 1;
    const code = raw.replace(/\r$/, "").split(";")[0].replace(/^[ \t]+|[ \t]+$/g, "");
    if (!code) return;
    const label = LABEL_LINE.exec(code);
    if (label) {
      if (labels.has(label[1])) errors.push({ line, message: `duplicate label '${label[1]}'` });
      else labels.set(label[1], instructions.length);
      return;
    }
    const [op, ...args] = code.split(/[ \t]+/);
    const problem = checkInstruction(op, args);
    if (problem) errors.push({ line, message: problem });
    else instructions.push({ op, args, line });
  });
  for (const ins of instructions) {
    if (["JMP", "JZ", "CALL"].includes(ins.op) && !labels.has(ins.args[0])) errors.push({ line: ins.line, message: `undefined label '${ins.args[0]}'` });
  }
  return { instructions, labels, errors };
}

/**
 * Checks the opcode, the operand count and a PUSH literal, returning a message or null.
 * @param op - the opcode word
 * @param args - the operand words
 */
function checkInstruction(op, args) {
  if (!Object.hasOwn(OPERANDS, op)) return `unknown opcode '${op}'`;
  const expected = OPERANDS[op];
  if (args.length !== expected) return expected === 0 ? `${op} expects no operands` : `${op} expects 1 operand`;
  if (op === "PUSH" && (!INTEGER.test(args[0]) || !inRange(BigInt(args[0])))) return `invalid integer '${args[0]}'`;
  return null;
}

/**
 * Returns true when a value fits in a signed 64 bit integer.
 * @param value - a BigInt
 */
function inRange(value) {
  return value >= MIN && value <= MAX;
}

/**
 * Returns the value or throws an overflow error.
 * @param value - the BigInt result
 * @param line - the line of the instruction
 */
function fit(value, line) {
  if (!inRange(value)) throw new VmError(line, "integer overflow");
  return value;
}

/**
 * Pops count values from the stack (last one popped is first in the result) or throws an underflow error.
 * @param stack - the data stack
 * @param count - how many values
 * @param line - the line of the instruction
 */
function take(stack, count, line) {
  if (stack.length < count) throw new VmError(line, "stack underflow");
  return stack.splice(stack.length - count, count);
}

/**
 * Applies an arithmetic or comparison opcode to the top two stack values.
 * @param op - ADD, SUB, MUL, DIV, MOD, LT or EQ
 * @param stack - the data stack
 * @param line - the line of the instruction
 */
function binary(op, stack, line) {
  const [a, b] = take(stack, 2, line);
  if ((op === "DIV" || op === "MOD") && b === 0n) throw new VmError(line, "division by zero");
  const results = { ADD: () => fit(a + b, line), SUB: () => fit(a - b, line), MUL: () => fit(a * b, line), DIV: () => fit(a / b, line), MOD: () => a % b, LT: () => (a < b ? 1n : 0n), EQ: () => (a === b ? 1n : 0n) };
  stack.push(results[op]());
}

/**
 * Runs the instructions and returns the text printed. On a runtime error the error carries the output so far.
 * @param program - { instructions, labels }
 */
function run(program) {
  const { instructions, labels } = program;
  const stack = [];
  const calls = [];
  const out = [];
  let pc = 0;
  let steps = 0;
  try {
    while (pc < instructions.length) {
      const { op, args, line } = instructions[pc];
      if (steps >= STEP_LIMIT) throw new VmError(line, "step limit exceeded");
      steps += 1;
      pc += 1;
      if (op === "HALT") break;
      pc = execute({ op, args, line, stack, calls, labels, out, pc });
    }
  } catch (error) {
    error.output = out.join("");
    throw error;
  }
  return out.join("");
}

/**
 * Executes one instruction and returns the next instruction index.
 * @param ctx - { op, args, line, stack, calls, labels, out, pc } where pc is already the next index
 */
function execute(ctx) {
  const { op, args, line, stack, calls, labels, out } = ctx;
  let pc = ctx.pc;
  if (op === "PUSH") stack.push(BigInt(args[0]));
  else if (op === "POP") take(stack, 1, line);
  else if (op === "DUP") { const [a] = take(stack, 1, line); stack.push(a, a); }
  else if (op === "SWAP") { const [a, b] = take(stack, 2, line); stack.push(b, a); }
  else if (op === "OVER") { const [a, b] = take(stack, 2, line); stack.push(a, b, a); }
  else if (op === "JMP") pc = labels.get(args[0]);
  else if (op === "JZ") { const [a] = take(stack, 1, line); if (a === 0n) pc = labels.get(args[0]); }
  else if (op === "CALL") {
    if (calls.length >= CALL_LIMIT) throw new VmError(line, "call stack overflow");
    calls.push(pc);
    pc = labels.get(args[0]);
  } else if (op === "RET") {
    if (calls.length === 0) throw new VmError(line, "return without call");
    pc = calls.pop();
  } else if (op === "PRINT") { const [a] = take(stack, 1, line); out.push(`${a}\n`); }
  else binary(op, stack, line);
  return pc;
}

/**
 * Entry point: parses the command line, runs the program and sets the exit code.
 * @param argv - the arguments after the script name
 */
function main(argv) {
  if (argv.length < 1) return finish("", "usage: node vm.mjs <program>\n", 2);
  let text;
  try {
    text = fs.readFileSync(argv[0], "utf8");
  } catch {
    return finish("", `error: cannot read '${argv[0]}'\n`, 2);
  }
  const program = parseProgram(text);
  if (program.errors.length) {
    const first = program.errors.reduce((best, e) => (e.line < best.line ? e : best));
    return finish("", `error: line ${first.line}: ${first.message}\n`, 1);
  }
  try {
    return finish(run(program), "", 0);
  } catch (error) {
    if (!(error instanceof VmError)) throw error;
    return finish(error.output, `error: line ${error.line}: ${error.message}\n`, 1);
  }
}

/**
 * Writes the output and error text and sets the exit code without cutting off pending output.
 * @param out - text for standard output
 * @param err - text for standard error
 * @param code - the exit code
 */
function finish(out, err, code) {
  if (out) process.stdout.write(out);
  if (err) process.stderr.write(err);
  process.exitCode = code;
}

main(process.argv.slice(2));
