-- postgresql
-- id	bigint Auto Increment
-- board_id bigint NOT NULL REFERENCES boards(id)
-- invite_name	text	
-- invite_hash	text NULL
CREATE TABLE board_access (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    board_id BIGINT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    invite_name text NOT NULL,
    invite_hash text NULL
);

