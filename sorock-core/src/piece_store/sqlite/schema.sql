create table if not exists sorockdb (
	id integer primary key,
	key text,
	idx integer,
	data blob
);
create index if not exists idx_key on sorockdb (key);