create table if not exists sorock (
	id integer primary key,
	key text,
	index integer,
	data blob
);
create index if not exists idx_socork_key on sorock(key);