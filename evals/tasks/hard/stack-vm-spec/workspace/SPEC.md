# SVM: a small stack language

`node vm.mjs <program file>` reads the program file, runs it, and writes what it prints to standard output.
`vm.mjs` is an ES module and uses only Node's built in modules.

## Program text

- The file is UTF-8 text. Lines end with LF or CRLF. The last line may have no line ending.
- Line numbers start at 1 and count every physical line, including blank lines and comment lines.
- A `;` starts a comment that runs to the end of the line. Remove the comment first.
- Then remove spaces and tabs from both ends of the line. A line that is empty now is skipped.
- A line that is exactly `NAME:` defines a label. NAME is one or more letters, digits or underscores and does not
  start with a digit. Names are case sensitive. The label stands for the next instruction after it. A label after the
  last instruction stands for the end of the program. Two labels may stand for the same place. A label must be alone on
  its line (after comments and spaces are removed).
- Every other line is one instruction: an opcode, then operands, separated by one or more spaces or tabs.
  Opcodes are upper case only. `push` is not an opcode.

## Values

All values are signed 64 bit integers, from -9223372036854775808 to 9223372036854775807. The program has one data
stack of values with no size limit, and one call stack of return addresses.

An integer literal is an optional `-` and then one or more decimal digits. `007` is 7 and `-0` is 0. These are not
integer literals: `+5`, `0x10`, `1_000`, `1.5`, `-`, an empty string, and any number outside the 64 bit range.

## Instructions

In the table, "pop b, pop a" means b is the top of the stack and a is the value below it.

| Instruction | Operands | Effect |
|---|---|---|
| `PUSH n` | 1 integer literal | push n |
| `POP` | none | remove the top value |
| `DUP` | none | push a copy of the top value |
| `SWAP` | none | swap the top two values |
| `OVER` | none | push a copy of the second value from the top (a b becomes a b a) |
| `ADD` | none | pop b, pop a, push a + b |
| `SUB` | none | pop b, pop a, push a - b |
| `MUL` | none | pop b, pop a, push a * b |
| `DIV` | none | pop b, pop a, push a / b rounded toward zero |
| `MOD` | none | pop b, pop a, push the remainder of a / b, which has the sign of a (7 MOD -2 is 1, -7 MOD 2 is -1) |
| `LT` | none | pop b, pop a, push 1 if a < b, otherwise 0 |
| `EQ` | none | pop b, pop a, push 1 if a equals b, otherwise 0 |
| `JMP label` | 1 label name | continue at the label |
| `JZ label` | 1 label name | pop a value; if it is 0, continue at the label, otherwise continue with the next instruction |
| `CALL label` | 1 label name | push the address of the next instruction on the call stack, then continue at the label |
| `RET` | none | pop an address from the call stack and continue there |
| `PRINT` | none | pop a value and write it in decimal followed by `\n` to standard output |
| `HALT` | none | stop the program at once |

## Running

1. The whole program is checked before anything runs (see "Errors found before running"). If it has an error,
   nothing runs and nothing is printed to standard output.
2. Execution starts at the first instruction. An instruction that does not jump is followed by the next one. The
   program ends normally when `HALT` runs or when execution moves past the last instruction. A normal end exits with
   code 0, whatever is left on the stack.
3. At most 1,000,000 instructions may run (every run of an instruction counts, including `HALT` and jumps). When 1,000,000
   have already run and another instruction is about to run, that is the error `step limit exceeded`, reported with the
   line of the instruction that was about to run.
4. The call stack holds at most 256 return addresses. A `CALL` when it already holds 256 is the error
   `call stack overflow`, reported with the line of that `CALL`.

## Errors

An error is written to standard error as `error: line N: MESSAGE` followed by `\n`, and the program exits with code 1.
N is the line number of the instruction involved. Whatever the program printed before a runtime error is still written
to standard output in full.

Errors found before running. Check every line first. If there are several errors, report only the one with the smallest
line number.

| Message | When | Line |
|---|---|---|
| `unknown opcode 'TOKEN'` | the first word of an instruction line is not an opcode (TOKEN is the word as written) | that line |
| `PUSH expects 1 operand` | wrong number of operands; the same text for `JMP`, `JZ` and `CALL` | that line |
| `POP expects no operands` | wrong number of operands; the same text for every opcode that takes none | that line |
| `invalid integer 'TOKEN'` | the operand of `PUSH` is not an integer literal | that line |
| `duplicate label 'NAME'` | a label is defined a second time | the line of the second definition |
| `undefined label 'NAME'` | the operand of `JMP`, `JZ` or `CALL` is not a defined label | the line of that instruction |

Errors found while running:

| Message | When |
|---|---|
| `stack underflow` | an instruction needs more values than the data stack holds |
| `division by zero` | `DIV` or `MOD` with b equal to 0 |
| `integer overflow` | the result of `ADD`, `SUB`, `MUL` or `DIV` is outside the 64 bit range (`DIV` of -9223372036854775808 by -1 is the only case for `DIV`) |
| `return without call` | `RET` with an empty call stack |
| `call stack overflow` | see Running, item 4 |
| `step limit exceeded` | see Running, item 3 |

## Command line

- With no program file argument, write `usage: node vm.mjs <program>` and `\n` to standard error and exit with code 2.
- When the file cannot be read, write `error: cannot read 'PATH'` and `\n` to standard error, with PATH as it was given
  on the command line, and exit with code 2.
- An empty program, or one with only comments, runs and exits with code 0 without printing anything.
- Output can be large (tens of thousands of lines). All of it must reach standard output before the process ends.

## Example

`examples/countdown.svm` prints 5, 4, 3, 2 and 1, and `examples/countdown.out` is its exact output.
