exports.FILE_MODE = 0755;

function nextTicket() {
  ticketCount = (typeof ticketCount === 'undefined' ? 0 : ticketCount) + 1;
  return ticketCount;
}

exports.nextTicket = nextTicket;
