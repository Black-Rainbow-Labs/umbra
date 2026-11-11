import { padLeft } from './strings.mjs';

export function money(cents) {
  return '$' + Math.floor(cents / 100) + '.' + padLeft(cents % 100, 2, '0');
}

export function percent(part, whole) {
  return whole === 0 ? '0%' : Math.round((part / whole) * 100) + '%';
}
