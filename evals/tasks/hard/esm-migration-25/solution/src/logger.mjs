export default function createLogger(prefix, sink = console.log) {
  return {
    info(message) { sink(`${prefix} INFO ${message}`); },
    warn(message) { sink(`${prefix} WARN ${message}`); },
  };
}
