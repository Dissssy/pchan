-- Posts are looked up by (board, post_number) everywhere: a thread link, a reply, a delete, and
-- now the batch "which of these did I write" check. Only the board half was indexed, so each
-- lookup walked every post on the board.
CREATE INDEX posts_board_number_idx ON posts (board, post_number);
