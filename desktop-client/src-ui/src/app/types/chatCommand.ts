export type ChatCommand = {
  id: string;
  kind: 'send_text';
  text: string;
};
