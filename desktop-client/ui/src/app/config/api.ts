/**
 * API 配置
 *
 * Desktop Client 使用嵌入式 IronClaw 服务器，端口与
 * desktop-client/src/embedded_server.rs 中的 EMBEDDED_SERVER_PORT 保持一致。
 */

export const EMBEDDED_SERVER_PORT = 38080;

export const API_BASE_URL = `http://localhost:${EMBEDDED_SERVER_PORT}`;
