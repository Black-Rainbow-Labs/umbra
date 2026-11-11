// Inputs and the expected outputs of the visible example for the newline-safe-generator task.

/**
 * Returns the visible input file (input/orders.csv) as CRLF text, with a trailing CRLF.
 */
export function visibleOrdersCsv() {
  const lines = [
    "id,customer,amount,status",
    "A-12,Zoë Martin,19.9,paid",
    'A-9,Alan Wu,"1,250.00",PAID',
    "A-101,Zoë Martin,5,Cancelled",
    "A-10,Łukasz Nowak,310.5,paid",
    "A-11,Alan Wu,0.75,shipped",
    "A-100,Łukasz Nowak,42,paid ",
  ];
  return lines.join("\r\n") + "\r\n";
}

/**
 * Returns the hidden input used to check the stated rules, with the line ending the caller asks for.
 * @param eol - "\r\n" or "\n"
 */
export function hiddenOrdersCsv(eol) {
  const lines = [
    "id,customer,amount,status",
    "B-7,Zoë Martin,2.675,Paid",
    'B-10,Łukasz Nowak,"1,234.50", PAID ',
    "B-2,  Alan Wu  ,12,shipped",
    "B-100,Çelik & Sons,0.999,cancelled",
    'B-99,李雷 (Li Lei),"1,000,000.4",refunded',
    "A-5,Zoë Martin,7.5,CANCELLED ",
    "B-5,Alan Wu,100.125,paid",
    "C-3,Łukasz Nowak,19.9, Cancelled",
    "",
    'D-8,Çelik & Sons,"3,500",paid',
  ];
  return lines.join(eol) + eol;
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
  };
}
