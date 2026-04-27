ALTER TABLE conversation_messages
ADD COLUMN attachments JSONB NOT NULL DEFAULT '[]'::jsonb;