import { capitalize } from '../strings.mjs';

export default function title(text) {
  return text.split(' ').map(capitalize).join(' ');
}
