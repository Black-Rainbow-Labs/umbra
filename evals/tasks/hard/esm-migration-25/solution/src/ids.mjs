let counter = 0;

export default function nextId(prefix = 'id') {
  counter += 1;
  return prefix + '-' + counter;
}
