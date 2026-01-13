-- Add desired_details column for declarative feature editing
-- This enables Terraform-style edit/diff/apply workflow

ALTER TABLE features ADD COLUMN desired_details TEXT;
