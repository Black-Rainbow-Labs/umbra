// Hidden cases for stack-vm-spec. Each group is one check; a group passes when every case in it matches
// standard output, standard error and the exit code exactly.

/** Joins lines into program text with LF line endings and a final newline. */
const prog = (...lines) => lines.join("\n") + "\n";
/** Builds a case that exits 0 with the given output. */
const ok = (name, program, stdout) => ({ name, program, stdout, stderr: "", code: 0 });
/** Builds a case that fails with a message on a line, after printing stdout. */
const fail = (name, program, line, message, stdout = "") => ({ name, program, stdout, stderr: `error: line ${line}: ${message}\n`, code: 1 });

const MAXV = "9223372036854775807";
const MINV = "-9223372036854775808";

/** The recursion program: depth n means n recursive calls below the first CALL. */
const recursion = (n) => prog("PUSH " + n, "CALL f", "PRINT", "HALT", "f:", "DUP", "JZ base", "PUSH 1", "SUB", "CALL f", "RET", "base:", "RET");

/** A loop that runs 5 * iterations + 5 instructions in total (PRINT at the end), plus extra instructions. */
const loopSteps = (iterations, extra) => prog("PUSH " + iterations, "loop:", "PUSH 1", "SUB", "DUP", "JZ done", "JMP loop", "done:", "PRINT", ...extra);

const bigOutput = [];
for (let i = 1; i <= 30000; i += 1) bigOutput.push(i);

export const GROUPS = [
  { name: "arithmetic and operand order", cases: [
    ok("all arithmetic", prog("PUSH 7", "PUSH 5", "ADD", "PRINT", "PUSH 7", "PUSH 5", "SUB", "PRINT", "PUSH 7", "PUSH 5", "MUL", "PRINT",
      "PUSH 7", "PUSH 2", "DIV", "PRINT", "PUSH -7", "PUSH 2", "DIV", "PRINT", "PUSH 7", "PUSH -2", "DIV", "PRINT",
      "PUSH -7", "PUSH 2", "MOD", "PRINT", "PUSH 7", "PUSH -2", "MOD", "PRINT", "PUSH -7", "PUSH -2", "MOD", "PRINT"),
      "12\n2\n35\n3\n-3\n-3\n-1\n1\n-1\n"),
    ok("order of operands", prog("PUSH 10", "PUSH 3", "SUB", "PRINT", "PUSH 20", "PUSH 4", "DIV", "PRINT", "PUSH 20", "PUSH 6", "MOD", "PRINT"), "7\n5\n2\n"),
    ok("zero results", prog("PUSH 0", "PUSH -5", "MUL", "PRINT", "PUSH 3", "PUSH 5", "DIV", "PRINT", "PUSH -3", "PUSH 5", "DIV", "PRINT", "PUSH -0", "PRINT"), "0\n0\n0\n0\n"),
  ] },
  { name: "stack operations", cases: [
    ok("swap", prog("PUSH 1", "PUSH 2", "SWAP", "PRINT", "PRINT"), "1\n2\n"),
    ok("over", prog("PUSH 1", "PUSH 2", "OVER", "PRINT", "PRINT", "PRINT"), "1\n2\n1\n"),
    ok("dup and pop", prog("PUSH 5", "DUP", "MUL", "PRINT", "PUSH 1", "PUSH 2", "POP", "PRINT"), "25\n1\n"),
    ok("three deep", prog("PUSH 1", "PUSH 2", "PUSH 3", "SWAP", "OVER", "PRINT", "PRINT", "PRINT", "PRINT"), "3\n2\n3\n1\n"),
  ] },
  { name: "comparisons", cases: [
    ok("lt", prog("PUSH 1", "PUSH 2", "LT", "PRINT", "PUSH 2", "PUSH 1", "LT", "PRINT", "PUSH 2", "PUSH 2", "LT", "PRINT", "PUSH -5", "PUSH 3", "LT", "PRINT"), "1\n0\n0\n1\n"),
    ok("eq", prog("PUSH 4", "PUSH 4", "EQ", "PRINT", "PUSH 4", "PUSH 5", "EQ", "PRINT", "PUSH -1", "PUSH -1", "EQ", "PRINT"), "1\n0\n1\n"),
  ] },
  { name: "64 bit values", cases: [
    ok("limits print exactly", prog(`PUSH ${MAXV}`, "PRINT", `PUSH ${MINV}`, "PRINT"), `${MAXV}\n${MINV}\n`),
    ok("large product is exact", prog("PUSH 3037000499", "DUP", "MUL", "PRINT"), "9223372030926249001\n"),
    ok("edges that do not overflow", prog(`PUSH ${MAXV}`, `PUSH ${MINV}`, "ADD", "PRINT", `PUSH ${MINV}`, "PUSH 1", "MUL", "PRINT", `PUSH ${MINV}`, "PUSH -1", "MOD", "PRINT", `PUSH ${MAXV}`, "PUSH 1", "DIV", "PRINT"),
      `-1\n${MINV}\n0\n${MAXV}\n`),
    ok("literals", prog("PUSH 007", "PRINT", "PUSH -0", "PRINT", "PUSH -007", "PRINT"), "7\n0\n-7\n"),
  ] },
  { name: "overflow errors", cases: [
    fail("add", prog(`PUSH ${MAXV}`, "PUSH 1", "ADD"), 3, "integer overflow"),
    fail("sub", prog(`PUSH ${MINV}`, "PUSH 1", "SUB"), 3, "integer overflow"),
    fail("mul", prog(`PUSH ${MAXV}`, "PUSH 2", "MUL"), 3, "integer overflow"),
    fail("mul negative", prog("PUSH 4611686018427387904", "PUSH -3", "MUL"), 3, "integer overflow"),
    fail("div min by minus one", prog("PUSH 1", "PRINT", `PUSH ${MINV}`, "PUSH -1", "DIV"), 5, "integer overflow", "1\n"),
  ] },
  { name: "jumps and loops", cases: [
    ok("example countdown", prog("; count down", "PUSH 5", "loop:", "  DUP", "  PRINT", "  PUSH 1", "  SUB", "  DUP", "  JZ done", "  JMP loop", "done:", "  POP"), "5\n4\n3\n2\n1\n"),
    ok("forward jump skips code", prog("PUSH 1", "JMP skip", "PUSH 99", "PRINT", "skip:", "PRINT"), "1\n"),
    ok("jz does not jump on nonzero and pops its value", prog("PUSH 7", "PUSH 3", "JZ no", "PRINT", "HALT", "no:", "PUSH 100", "PRINT"), "7\n"),
    ok("label at the end of the program", prog("PUSH 1", "PRINT", "JMP end", "PUSH 2", "PRINT", "end:"), "1\n"),
    ok("two labels for one place and case sensitive names", prog("JMP b", "PUSH 9", "PRINT", "a:", "b:", "PUSH 2", "PRINT", "Loop:", "loop:"), "2\n"),
    ok("100000 iterations", prog("PUSH 100000", "loop:", "PUSH 1", "SUB", "DUP", "JZ done", "JMP loop", "done:", "PRINT"), "0\n"),
  ] },
  { name: "call and return", cases: [
    ok("nested calls", prog("CALL a", "PUSH 3", "PRINT", "HALT", "a:", "PUSH 1", "PRINT", "CALL b", "PUSH 2", "PRINT", "RET", "b:", "PUSH 9", "PRINT", "RET"), "1\n9\n2\n3\n"),
    ok("factorial of 10", prog("PUSH 10", "CALL fact", "PRINT", "HALT", "fact:", "DUP", "JZ base", "DUP", "PUSH 1", "SUB", "CALL fact", "MUL", "RET", "base:", "POP", "PUSH 1", "RET"), "3628800\n"),
    ok("fibonacci of 15", prog("PUSH 15", "CALL fib", "PRINT", "HALT", "fib:", "DUP", "PUSH 2", "LT", "JZ rec", "RET", "rec:", "DUP", "PUSH 1", "SUB", "CALL fib", "SWAP", "PUSH 2", "SUB", "CALL fib", "ADD", "RET"), "610\n"),
    ok("recursion 256 deep is allowed", recursion(255), "0\n"),
    ok("halt inside a call", prog("CALL a", "PUSH 5", "PRINT", "a:", "PUSH 4", "PRINT", "HALT"), "4\n"),
    ok("same routine called twice", prog("PUSH 2", "CALL double", "CALL double", "PRINT", "HALT", "double:", "DUP", "ADD", "RET"), "8\n"),
  ] },
  { name: "text format, comments and line endings", cases: [
    ok("comments, blanks, tabs and spaces", "; header\n\n   \nPUSH   1   ; one\n\tPUSH\t2\t;two\nADD ;sum\n  PRINT\n;end", "3\n"),
    ok("crlf line endings", ["PUSH 5", "loop:", "DUP", "PRINT", "PUSH 1", "SUB", "DUP", "JZ done", "JMP loop", "done:", "POP"].join("\r\n") + "\r\n", "5\n4\n3\n2\n1\n"),
    ok("no newline at the end", "PUSH 8\nPRINT", "8\n"),
    ok("label with a comment", prog("JMP go ; jump", "go: ; here", "PUSH 3", "PRINT"), "3\n"),
    ok("empty program", "", ""),
    ok("only comments and blanks", "; nothing\n\n; here\n", ""),
    fail("line numbers count comments and blanks", "; header\n \nPUSH 1   ; one\n\tPUSH 2\nADD\nPRINT\nPRINT\n", 7, "stack underflow", "3\n"),
  ] },
  { name: "stack underflow", cases: [
    fail("pop", prog("POP"), 1, "stack underflow"),
    fail("dup", prog("PUSH 1", "POP", "DUP"), 3, "stack underflow"),
    fail("swap with one value", prog("PUSH 1", "SWAP"), 2, "stack underflow"),
    fail("over with one value", prog("PUSH 1", "OVER"), 2, "stack underflow"),
    fail("add with one value", prog("PUSH 1", "ADD"), 2, "stack underflow"),
    fail("sub", prog("SUB"), 1, "stack underflow"),
    fail("mul", prog("PUSH 2", "MUL"), 2, "stack underflow"),
    fail("div", prog("PUSH 2", "DIV"), 2, "stack underflow"),
    fail("mod", prog("MOD"), 1, "stack underflow"),
    fail("lt", prog("PUSH 2", "LT"), 2, "stack underflow"),
    fail("eq", prog("EQ"), 1, "stack underflow"),
    fail("jz", prog("JZ x", "x:"), 1, "stack underflow"),
    fail("print after earlier output", prog("PUSH 4", "PRINT", "PRINT"), 3, "stack underflow", "4\n"),
  ] },
  { name: "division by zero", cases: [
    fail("div", prog("PUSH 1", "PRINT", "PUSH 1", "PUSH 0", "DIV"), 5, "division by zero", "1\n"),
    fail("mod", prog("PUSH 0", "PUSH 0", "MOD"), 3, "division by zero"),
    ok("zero numerator is fine", prog("PUSH 0", "PUSH 5", "DIV", "PRINT", "PUSH 0", "PUSH 5", "MOD", "PRINT"), "0\n0\n"),
  ] },
  { name: "errors found before running", cases: [
    fail("unknown opcode, nothing printed first", prog("PUSH 1", "PRINT", "push 2"), 3, "unknown opcode 'push'"),
    fail("unknown opcode HLT", prog("PUSH 1", "HLT"), 2, "unknown opcode 'HLT'"),
    fail("label followed by an instruction", prog("loop: PUSH 1"), 1, "unknown opcode 'loop:'"),
    fail("push without operand", prog("PUSH"), 1, "PUSH expects 1 operand"),
    fail("push with two operands", prog("PUSH 1 2"), 1, "PUSH expects 1 operand"),
    fail("pop with an operand", prog("PUSH 1", "POP 1"), 2, "POP expects no operands"),
    fail("print with an operand", prog("PUSH 1", "PRINT x"), 2, "PRINT expects no operands"),
    fail("jmp without a label", prog("JMP"), 1, "JMP expects 1 operand"),
    fail("call with two labels", prog("a:", "CALL a a"), 2, "CALL expects 1 operand"),
    fail("jz with no label", prog("PUSH 0", "JZ"), 2, "JZ expects 1 operand"),
    fail("invalid integer word", prog("PUSH abc"), 1, "invalid integer 'abc'"),
    fail("invalid integer fraction", prog("PUSH 1.5"), 1, "invalid integer '1.5'"),
    fail("invalid integer plus sign", prog("PUSH +5"), 1, "invalid integer '+5'"),
    fail("invalid integer hex", prog("PUSH 0x10"), 1, "invalid integer '0x10'"),
    fail("invalid integer lone minus", prog("PUSH -"), 1, "invalid integer '-'"),
    fail("integer above the range", prog("PUSH 9223372036854775808"), 1, "invalid integer '9223372036854775808'"),
    fail("integer below the range", prog("PUSH -9223372036854775809"), 1, "invalid integer '-9223372036854775809'"),
    fail("duplicate label", prog("a:", "PUSH 1", "; x", "a:", "PRINT"), 4, "duplicate label 'a'"),
    fail("undefined label", prog("PUSH 1", "PRINT", "JMP nowhere"), 3, "undefined label 'nowhere'"),
    fail("label names are case sensitive", prog("JMP Loop", "loop:"), 1, "undefined label 'Loop'"),
    fail("undefined call target", prog("PUSH 1", "PRINT", "CALL missing", "HALT"), 3, "undefined label 'missing'"),
    fail("smallest line wins: undefined label first", prog("PUSH 1", "JZ nope", "PUSH 2", "PUSH 3", "bogus"), 2, "undefined label 'nope'"),
    fail("smallest line wins: unknown opcode first", prog("PUSH 1", "bogus", "PUSH 2", "JMP nope"), 2, "unknown opcode 'bogus'"),
    fail("smallest line wins: duplicate label first", prog("a:", "a:", "PUSH x"), 2, "duplicate label 'a'"),
  ] },
  { name: "limits and return errors", cases: [
    fail("return without call", prog("PUSH 1", "RET"), 2, "return without call"),
    fail("return after the calls ended", prog("CALL a", "RET", "a:", "RET"), 2, "return without call"),
    fail("recursion 257 deep overflows", recursion(256), 10, "call stack overflow"),
    fail("infinite loop hits the step limit", prog("PUSH 1", "PRINT", "loop:", "JMP loop"), 4, "step limit exceeded", "1\n"),
    ok("exactly 1000000 instructions is allowed", loopSteps(199999, ["PUSH 0", "POP", "PUSH 0", "POP"]), "0\n"),
    fail("the 1000001st instruction is refused", loopSteps(199999, ["PUSH 0", "POP", "PUSH 0", "POP", "PUSH 0"]), 14, "step limit exceeded", "0\n"),
  ] },
  { name: "halt and normal ends", cases: [
    ok("halt stops the program", prog("PUSH 1", "PRINT", "HALT", "PUSH 2", "PRINT"), "1\n"),
    ok("left over values are fine", prog("PUSH 1", "PUSH 2", "PUSH 3"), ""),
    ok("halt with an operand-free label after it", prog("HALT", "end:"), ""),
  ] },
  { name: "command line and large output", cases: [
    { name: "no arguments", args: [], noFile: true, stdout: "", stderr: "usage: node vm.mjs <program>\n", code: 2 },
    { name: "missing file", args: ["missing.svm"], noFile: true, stdout: "", stderr: "error: cannot read 'missing.svm'\n", code: 2 },
    ok("30000 lines of output arrive in full", prog("PUSH 0", "loop:", "PUSH 1", "ADD", "DUP", "PRINT", "DUP", "PUSH 30000", "EQ", "JZ loop"), bigOutput.join("\n") + "\n"),
    { name: "output before an error is kept in full", program: prog("PUSH 0", "loop:", "PUSH 1", "ADD", "DUP", "PRINT", "DUP", "PUSH 20000", "EQ", "JZ loop", "POP", "POP"), stdout: bigOutput.slice(0, 20000).join("\n") + "\n", stderr: "error: line 12: stack underflow\n", code: 1 },
  ] },
];
