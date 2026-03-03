PRAGMA foreign_keys=OFF;--> statement-breakpoint
CREATE TABLE `__new_progress` (
	`user_id` integer NOT NULL,
	`book_id` integer NOT NULL,
	`chapter_path` text NOT NULL,
	`position_sec` real DEFAULT 0 NOT NULL,
	`updated_at` text DEFAULT (datetime('now')) NOT NULL,
	PRIMARY KEY(`user_id`, `book_id`, `chapter_path`),
	FOREIGN KEY (`user_id`) REFERENCES `users`(`id`) ON UPDATE no action ON DELETE no action,
	FOREIGN KEY (`book_id`) REFERENCES `books`(`id`) ON UPDATE no action ON DELETE no action
);
--> statement-breakpoint
INSERT INTO `__new_progress`("user_id", "book_id", "chapter_path", "position_sec", "updated_at") SELECT "user_id", "book_id", "chapter_path", "position_sec", "updated_at" FROM `progress`;--> statement-breakpoint
DROP TABLE `progress`;--> statement-breakpoint
ALTER TABLE `__new_progress` RENAME TO `progress`;--> statement-breakpoint
PRAGMA foreign_keys=ON;