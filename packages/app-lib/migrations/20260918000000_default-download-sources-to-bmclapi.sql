-- Prefer the mainland mirror for Minecraft metadata, files, loaders, and Java.
--
-- Rows that still hold the previous default (`auto`) move to the explicit
-- mirror-first choice, which the launcher resolves through OpenBMCLAPI while
-- keeping the official host as a fallback. A source the player picked
-- deliberately is left untouched.
UPDATE settings
SET
	minecraft_metadata_source = CASE
		WHEN minecraft_metadata_source = 'auto' THEN 'mirror_preferred'
		ELSE minecraft_metadata_source
	END,
	minecraft_file_source = CASE
		WHEN minecraft_file_source = 'auto' THEN 'mirror_preferred'
		ELSE minecraft_file_source
	END;
