create table regions
(
    id   uuid primary key,
    name text not null
);

create table regions_tags
(
    region uuid not null references regions,
    key    text not null,
    value  text,
    primary key (region, key)
);

create table if not exists zones
(
    id     uuid primary key,
    name   text not null,
    region uuid references regions
);

create table zones_tags
(
    zone  uuid not null references zones,
    key   text not null,
    value text,
    primary key (zone, key)
);

create table if not exists agents
(
    id               text primary key,
    zone             uuid        not null references zones,
    endpoint         text        not null,
    token            text        not null,
    added_at         timestamptz not null,
    healthchecked_at timestamptz not null
);

create index if not exists agents_healthchecked_at_idx on agents (healthchecked_at);

create table if not exists agents_tags
(
    agent text not null references agents,
    key   text not null,
    value text,
    primary key (agent, key)
);

create table tenants
(
    id uuid primary key
);

create table if not exists users
(
    id       uuid primary key references tenants,
    email    text unique not null,
    password text        not null
);

create index if not exists users_email_idx on users (email);

create table users_tags
(
    "user" uuid not null references users,
    key    text not null,
    value  text,
    primary key ("user", key)
);

create table if not exists organisations
(
    id   uuid primary key references tenants,
    name text not null
);

create table organisations_tags
(
    organisation uuid not null references organisations,
    key          text not null,
    value        text,
    primary key (organisation, key)
);

create table if not exists organisations_members
(
    organisation uuid not null references organisations,
    "user"       uuid not null references users,
    primary key (organisation, "user")
);

create table checks
(
    id       uuid primary key,
    interval interval not null,
    name     text     not null,
    tenant   uuid references tenants,
    kind     jsonb    not null
);

create table checks_zones
(
    "check" uuid not null references checks,
    zone    uuid not null references zones,
    primary key ("check", zone)
);
