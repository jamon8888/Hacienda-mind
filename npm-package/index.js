const path = require("node:path");
const os = require("node:os");

const binaryName = os.type() === "Windows_NT" ? "hacienda-mcp.exe" : "hacienda-mcp";

module.exports = {
  binaryPath: path.join(__dirname, "bin", binaryName),
};
