ALTER TABLE `books` ADD `fingerprint` text;--> statement-breakpoint
CREATE UNIQUE INDEX `books_fingerprint_unique` ON `books` (`fingerprint`);