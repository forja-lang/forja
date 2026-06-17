class MultiLevelEncodingError extends Error {
  constructor() {
    super("Multi-level URL encoding is not allowed");
    this.name = "MultiLevelEncodingError";
  }
}
function validateAndDecodePathname(pathname) {
  let decoded;
  try {
    decoded = decodeURI(pathname);
  } catch (_e) {
    throw new Error("Invalid URL encoding");
  }
  let iterations = 0;
  while (decoded !== pathname && iterations < 10) {
    pathname = decoded;
    try {
      decoded = decodeURI(pathname);
    } catch {
      break;
    }
    iterations++;
  }
  return decoded;
}
export {
  MultiLevelEncodingError,
  validateAndDecodePathname
};
