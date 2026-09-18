-- Restore the health-ranked default for the Minecraft download sources.
--
-- The previous default pointed every Minecraft metadata, file, loader and Java
-- request at the mainland mirror first, whatever that mirror's recent health
-- was. Downloads that used to start from the official host and fall back only
-- when a route was unhealthy became slow, kept their download slots busy, and
-- then failed while waiting for a slot. Rows that still hold the mirror default
-- go back to the previous choice; a source the player picked is left untouched.
UPDATE settings
SET
	minecraft_metadata_source = CASE
		WHEN minecraft_metadata_source = 'mirror_preferred' THEN 'auto'
		ELSE minecraft_metadata_source
	END,
	minecraft_file_source = CASE
		WHEN minecraft_file_source = 'mirror_preferred' THEN 'auto'
		ELSE minecraft_file_source
	END;
