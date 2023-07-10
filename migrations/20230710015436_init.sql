-- Add migration script here
-- Created by `sqlx migrate add init` command
create table todos
(
    id        SERIAL Primary Key,
    text      text    not null,
    completed boolean not null default false
)