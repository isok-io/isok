create type owner_type as enum ('user', 'organization');


create table organizations
(
    organization_id uuid primary key not null default gen_random_uuid(),
    name            text             not null
);

create table users
(
    user_id       uuid primary key         not null default gen_random_uuid(),
    username      text                     not null,
    password      text                     not null,
    email_address text unique              not null,
    created_at    timestamp with time zone not null,
    updated_at    timestamp with time zone not null,
    deleted_at    timestamp with time zone          default null
);

create table users_organizations
(
    organization_id uuid not null,
    user_id         uuid not null,
    primary key (organization_id, user_id),
    foreign key (organization_id) references organizations (organization_id)
        match simple on update no action on delete no action,
    foreign key (user_id) references users (user_id)
        match simple on update no action on delete no action
);

create table owners
(
    owner_id        uuid primary key         not null default gen_random_uuid(),
    kind            owner_type               not null default 'user'::owner_type,
    user_id         uuid,
    organization_id uuid,
    created_at      timestamp with time zone not null,
    updated_at      timestamp with time zone not null,
    deleted_at      timestamp with time zone,
    foreign key (organization_id) references organizations (organization_id)
        match simple on update no action on delete no action,
    foreign key (user_id) references users (user_id)
        match simple on update no action on delete no action
);
