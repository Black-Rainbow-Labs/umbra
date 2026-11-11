export function parseCsv(text) {
  const lines = text.split('\n').filter((line) => line.trim() !== '');
  const [head, ...rest] = lines;
  const columns = head.split(',');
  return rest.map((line) => {
    const cells = line.split(',');
    const row = {};
    columns.forEach((column, i) => { row[column] = cells[i] ?? ''; });
    return row;
  });
}

export function toCsv(rows, columns) {
  const lines = [columns.join(',')];
  for (const row of rows) lines.push(columns.map((column) => row[column] ?? '').join(','));
  return lines.join('\n') + '\n';
}
