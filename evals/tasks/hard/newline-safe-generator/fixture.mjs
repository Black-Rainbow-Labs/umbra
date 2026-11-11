// Inputs and the expected outputs of the visible example for the newline-safe-generator task.

/**
 * Returns the visible input file (input/orders.csv) as CRLF text, with a trailing CRLF.
 */
export function visibleOrdersCsv() {
  const lines = [
    "id,customer,amount,status,date",
    "A-12,Zoë Martin,19.9,paid,2024-03-14",
    'A-9,Alan Wu,"1,250.00",PAID,2024-01-05',
    "A-101,Zoë Martin,5,Cancelled,2024-03-20",
    "A-10,Łukasz Nowak,310.5,paid,2024-01-28",
    "A-11,Alan Wu,0.75,shipped,",
    "A-100,Łukasz Nowak,42,paid ,2024-03-02",
  ];
  return lines.join("\r\n") + "\r\n";
}

/**
 * Returns the hidden input used to check the stated rules, with the line ending the caller asks for. It holds
 * cancelled and refunded orders, a blank line, a customer name that needs quoting, ids that do not end in a
 * number, bad dates and four rows that are skipped (rows 12 to 15).
 * @param eol - "\r\n" or "\n"
 */
export function hiddenOrdersCsv(eol) {
  const lines = [
    "id,customer,amount,status,date",
    "B-7,Zoë Martin,2.675,Paid,2024-02-11",
    'B-10,Łukasz Nowak,"1,234.50", PAID ,2024-02-29',
    "B-2,  Alan Wu  ,12,shipped,2023-12-31",
    "B-100,Çelik & Sons,0.999,cancelled,2024-02-01",
    'B-99,李雷 (Li Lei),"1,000,000.4",refunded,2024-03-05',
    "A-5,Zoë Martin,7.5,CANCELLED ,",
    "B-5,Alan Wu,100.125,paid,2024-02-11",
    "C-3,Łukasz Nowak,19.9, Cancelled,2024-13-01",
    "",
    'D-8,Çelik & Sons,"3,500",paid,',
    'MISC,"Smith, ""Jo"" Lee",8.005,paid,2024-02-15',
    "Z-x7,Alan Wu,1,paid,bad date",
    "E-4,Zoë Martin,abc,paid,2024-02-01",
    ",Nobody,5,paid,2024-02-01",
    "F-1,Alan Wu,-5,paid,2024-02-01",
    'F-2,Alan Wu,"$5.00",paid,2024-02-01',
  ];
  return lines.join(eol) + eol;
}

/** The stderr lines the generator must write for the skipped rows of hiddenOrdersCsv, in order. */
export const HIDDEN_SKIPPED = "skipped row 12: invalid amount\nskipped row 13: missing id\nskipped row 14: invalid amount\nskipped row 15: invalid amount\n";

/**
 * Returns a second hidden input with a byte order mark, CRLF line endings, a customer name that holds a line break
 * and a tab, and a refund.
 */
export function messyOrdersCsv() {
  const lines = [
    "id,customer,amount,status,date",
    'G-1,"Anna\r\n  Ace",10,paid,2024-05-01',
    'G-2,"Anna\tAce",5.5,paid,2024-05-02',
    "G-3,  Bob   Stone ,3,refunded,2024-05-03",
    "G-4,Bob Stone,3.5,paid,2024-06-01",
  ];
  return "﻿" + lines.join("\r\n") + "\r\n";
}

/** Returns an input with only a header row. */
export function headerOnlyCsv() {
  return "id,customer,amount,status,date\n";
}

/**
 * Returns the expected outputs for the visible input, keyed by file name. These are the texts shown in the prompt.
 */
export function visibleExpected() {
  return {
    "summary.txt": "orders: 5\ncustomers: 3\ntotal: 1623.15\nlargest: A-9 (1250.00)\n",
    "orders.tsv": [
      "id\tcustomer\tamount\tstatus",
      "A-9\tAlan Wu\t1250.00\tpaid",
      "A-10\tŁukasz Nowak\t310.50\tpaid",
      "A-11\tAlan Wu\t0.75\tshipped",
      "A-12\tZoë Martin\t19.90\tpaid",
      "A-100\tŁukasz Nowak\t42.00\tpaid",
      "A-101\tZoë Martin\t5.00\tcancelled",
      "",
    ].join("\n"),
    "by_customer.json": [
      "{",
      '  "Alan Wu": {',
      '    "orders": 2,',
      '    "total": "1250.75"',
      "  },",
      '  "Zoë Martin": {',
      '    "orders": 1,',
      '    "total": "19.90"',
      "  },",
      '  "Łukasz Nowak": {',
      '    "orders": 2,',
      '    "total": "352.50"',
      "  }",
      "}",
      "",
    ].join("\n"),
    "report.md": [
      "# Order report",
      "",
      "| id | customer | amount |",
      "|---|---|---|",
      "| A-9 | Alan Wu | 1250.00 |",
      "| A-10 | Łukasz Nowak | 310.50 |",
      "| A-11 | Alan Wu | 0.75 |",
      "| A-12 | Zoë Martin | 19.90 |",
      "| A-100 | Łukasz Nowak | 42.00 |",
      "",
      "Total: 1623.15",
      "",
    ].join("\n"),
    "customers.csv": ["customer,orders,total", "Alan Wu,2,1250.75", "Łukasz Nowak,2,352.50", "Zoë Martin,1,19.90", ""].join("\n"),
    "by_month.csv": ["month,orders,total", "2024-01,2,1560.50", "2024-03,2,61.90", "unknown,1,0.75", ""].join("\n"),
  };
}
