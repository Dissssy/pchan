-- add a boards column to the spoilers table, it's a nullable BigInt[]
ALTER TABLE spoilers ADD COLUMN boards BigInt[] NULL;

-- also rename the img column to img_path
ALTER TABLE spoilers RENAME COLUMN img TO img_path;