export default function (pi) {
  pi.registerMcpServer("fence_exa", {
    url: "https://mcp.exa.ai/mcp",
    headers: { Authorization: "" },
    exposure: "direct",
  });
}
