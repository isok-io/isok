create table checks (
  check_id uuid primary key not null default gen_random_uuid(),
  owner_id uuid not null,
  kind jsonb not null,
  max_latency interval not null,
  interval interval not null,
  region character varying not null,
  created_at timestamp with time zone not null,
  updated_at timestamp with time zone not null,
  deleted_at timestamp with time zone
);
