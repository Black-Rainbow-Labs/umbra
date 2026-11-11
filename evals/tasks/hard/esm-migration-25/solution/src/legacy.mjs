export const FILE_MODE = 0o755;

let ticketCount;

export function nextTicket() {
  ticketCount = (typeof ticketCount === 'undefined' ? 0 : ticketCount) + 1;
  return ticketCount;
}
