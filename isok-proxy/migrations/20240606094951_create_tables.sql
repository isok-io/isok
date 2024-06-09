create type organisation_type as enum ('user', 'normal');
create type organisation_user_role as enum ('owner', 'member');


create table organizations
(
    organization_id uuid primary key         not null default gen_random_uuid(),
    type            organisation_type        not null,
    name            text,
    created_at      timestamp with time zone not null,
    updated_at      timestamp with time zone not null,
    deleted_at      timestamp with time zone          default null
);

create table organizations_tags
(
    organization_id uuid not null references organizations,
    key             text not null,
    value           text,
    primary key (organization_id, key)
);

create table users
(
    user_id           uuid primary key         not null default gen_random_uuid(),
    username          text                     not null,
    password          text                     not null,
    email_address     text unique              not null,
    self_organization uuid unique              not null references organizations match simple on update no action on delete no action,
    created_at        timestamp with time zone not null,
    updated_at        timestamp with time zone not null,
    deleted_at        timestamp with time zone          default null
);

create table users_tags
(
    user_id uuid not null references users,
    key     text not null,
    value   text,
    primary key (user_id, key)
);

create table users_organizations
(
    user_id         uuid                   not null references users match simple on update no action on delete no action,
    organization_id uuid                   not null references organizations match simple on update no action on delete no action,
    role            organisation_user_role not null default 'member'::organisation_user_role,
    primary key (organization_id, user_id)
);
