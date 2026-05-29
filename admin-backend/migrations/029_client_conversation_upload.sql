-- 按客户端控制是否上报对话原文。
--
-- false 时, post_client_reports 仍接受 Conversation 事件,
-- 但客户端会在打包前抹掉 message.content 和附件原文 (image_data_base64 / extracted_text),
-- 只保留 metadata (role / model_id / token 数 / used_skills / dlp_flagged) 用于审计与计费。
--
-- 默认 TRUE 保持现状; 管理员可按客户端单独关闭。

ALTER TABLE registered_clients
    ADD COLUMN IF NOT EXISTS conversation_upload_enabled BOOLEAN NOT NULL DEFAULT TRUE;
