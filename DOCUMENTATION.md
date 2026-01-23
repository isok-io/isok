# isok Documentation

## Table of Contents

1. [Functional Architecture](#functional-architecture)
2. [Data Flow](#data-flow)
3. [API Reference](#api-reference)
4. [Data Models](#data-models)
5. [Pulsar Topics](#pulsar-topics)
6. [Database Schema](#database-schema)

---

## Functional Architecture

### System Overview

isok is designed as a distributed monitoring system with the following key principles:

1. **Multi-region deployment**: Each region has its own API and agents
2. **Multi-tenancy**: Organizations can manage their own checks independently
3. **Horizontal scalability**: Multiple agents can run per region
4. **Decoupled components**: Pulsar message queue enables async communication

### Component Responsibilities

#### isok-proxy (API Gateway)

The proxy is the single entry point for all client requests. It handles:

- **Authentication**: Token based auth using Biscuit tokens
- **User Management**: CRUD operations for users
- **Organization Management**: Multi-tenant organization support with roles (owner/member)
- **Request Routing**: Forwards check-related requests to appropriate regional APIs
- **Authorization**: Validates user permissions for organizations

#### isok-api (Regional API)

Each region runs one isok-api instance that:

- **Stores Check Definitions**: Persists checks in PostgreSQL
- **Publishes Commands**: Sends add/remove commands to Pulsar for agents
- **Serves Check Data**: Responds to queries from the proxy

#### isok-agent (Monitoring Agent)

Agents are the workers that execute monitoring checks:

- **Job Scheduling**: Uses a time-wheel scheduler for efficient scheduling
- **Concurrent Execution**: Runs multiple checks in parallel using task pools
- **HTTP Client Pool**: Reuses HTTP clients via MagicPool for efficiency
- **Result Reporting**: Sends check results to Pulsar

#### isok-offloader (Data Processor)

The offloader processes check results:

- **Warp10 Sink**: Writes raw metrics to Warp10 time-series database
- **Aggregator**: Aggregates results from multiple agents per check
- **Dual Output**: Produces both raw and aggregated data

---

## Data Flow

### Check Creation Flow

```
1. Client -> POST /checks/{org_id} -> isok-proxy
2. isok-proxy validates auth -> forwards to isok-api (by region)
3. isok-api -> INSERT into PostgreSQL
4. isok-api -> Publish "Add" command to Pulsar
5. isok-agent consumes command -> schedules check
```

### Check Execution Flow

```
1. isok-agent scheduler triggers check
2. Agent executes HTTP request
3. Agent -> Publish result to Pulsar (http topic)
4. isok-offloader consumes result
5. offloader -> Write to Warp10 (raw metrics)
6. offloader -> Aggregate results
7. offloader -> Publish to Pulsar (aggregated-http topic)
```

### Check Deletion Flow

```
1. Client -> DELETE /checks/{org_id}/{id} -> isok-proxy
2. isok-proxy validates auth -> forwards to isok-api
3. isok-api -> Soft DELETE in PostgreSQL
4. isok-api -> Publish "Remove" command to Pulsar
5. isok-agent consumes command -> removes from scheduler
```

---

## API Reference

### Authentication

#### POST /login
Authenticate user and receive a Biscuit token.

**Request Body:**
```json
{
  "login": "user@example.com",
  "password": "password123"
}
```

**Response:** Biscuit token string

**Headers for authenticated requests:**
```
Authorization: Bearer <token>
```

---

### Users

#### GET /users
List all users (requires authentication).

**Response:**
```json
[
  {
    "owner_id": "uuid",
    "username": "john_doe",
    "email_address": "john@example.com"
  }
]
```

#### GET /users/{id}
Get a specific user.

#### POST /users
Create a new user.

**Request Body:**
```json
{
  "username": "john_doe",
  "password": "securepassword",
  "email_address": "john@example.com"
}
```

#### PUT /users/{id}/rename
Change username.

**Request Body:**
```json
{
  "username": "new_username"
}
```

#### PUT /users/{id}/email
Change email address.

**Request Body:**
```json
{
  "email_address": "new@example.com"
}
```

#### PUT /users/{id}/password
Change password.

**Request Body:**
```json
{
  "old_password": "current_password",
  "new_password": "new_password",
  "confirm_password": "new_password"
}
```

#### DELETE /users/{id}
Delete user account.

---

### Organizations

#### GET /organizations
List organizations the current user belongs to.

**Response:**
```json
[
  {
    "organization_id": "uuid",
    "tags": {},
    "name": "My Organization",
    "users": [
      {
        "role": "owner",
        "owner_id": "uuid",
        "username": "john_doe",
        "email_address": "john@example.com"
      }
    ]
  }
]
```

#### GET /organizations/{id}
Get a specific organization.

#### POST /organizations
Create a new organization.

**Request Body:**
```json
{
  "name": "New Organization"
}
```

#### DELETE /organizations/{id}
Delete an organization (owner only).

#### POST /organizations/{id}/members
Add a member to organization.

**Request Body:**
```json
{
  "email": "user@example.com",
  "role": "member"
}
```

#### PUT /organizations/{id}/members/{user_id}
Change member role.

**Request Body:**
```json
"owner"
```
or
```json
"member"
```

#### DELETE /organizations/{id}/members/{user_id}
Remove member from organization.

---

### Checks

The `organization_id` path parameter can be:
- A UUID for a specific organization
- `me` to use the user's personal organization

#### GET /checks/{organization_id}
List all checks for an organization.

**Response:**
```json
[
  {
    "id": "uuid",
    "owner_id": "uuid",
    "kind": {
      "type": "http",
      "data": {
        "uri": "https://example.com/health",
        "headers": {}
      }
    },
    "max_latency": { "secs": 5, "nanos": 0 },
    "interval": 60,
    "region": "eu-fr-1"
  }
]
```

#### GET /checks/{organization_id}/{id}
Get a specific check.

#### POST /checks/{organization_id}
Create a new check.

**Request Body:**
```json
{
  "owner_id": "organization-uuid",
  "kind": {
    "type": "http",
    "data": {
      "uri": "https://example.com/health",
      "headers": {
        "Authorization": "Biscuit token"
      }
    }
  },
  "max_latency": { "secs": 5, "nanos": 0 },
  "interval": 60,
  "region": "eu-fr-1"
}
```

**Check Kind Types:**

HTTP:
```json
{
  "type": "http",
  "data": {
    "uri": "https://example.com",
    "headers": {}
  }
}
```

TCP:
```json
{
  "type": "tcp",
  "data": {
    "host": { "Domain": "example.com" },
    "port": 443
  }
}
```

DNS:
```json
{
  "type": "dns",
  "data": {
    "domain": "example.com",
    "dns_server": null
  }
}
```

ICMP:
```json
{
  "type": "icmp",
  "data": {
    "host": { "IpAddr": "1.2.3.4" }
  }
}
```

#### PUT /checks/{organization_id}/{id}/kind
Update check kind/target.

**Request Body:** Same as `kind` in create

#### PUT /checks/{organization_id}/{id}/interval
Update check interval.

**Request Body:**
```json
{ "secs": 30, "nanos": 0 }
```

#### PUT /checks/{organization_id}/{id}/max_latency
Update maximum acceptable latency.

**Request Body:**
```json
{ "secs": 10, "nanos": 0 }
```

#### DELETE /checks/{organization_id}/{id}
Delete a check.

---

### Utility Endpoints

#### GET /ping
Health check endpoint.

**Response:** `PONG !`

---

## Data Models

### Check

| Field | Type | Description |
|-------|------|-------------|
| `check_id` | UUID | Unique identifier |
| `owner_id` | UUID | Organization that owns this check |
| `kind` | CheckKind | Type and configuration of check |
| `max_latency` | Duration | Maximum acceptable response time |
| `interval` | Duration | Time between check executions |
| `region` | String | Region where check runs |
| `created_at` | DateTime | Creation timestamp |
| `updated_at` | DateTime | Last update timestamp |
| `deleted_at` | DateTime? | Soft delete timestamp |

### User

| Field | Type | Description |
|-------|------|-------------|
| `user_id` | UUID | Unique identifier |
| `username` | String | Display name (3-32 ASCII chars) |
| `password` | String | Argon2 hashed password |
| `email_address` | String | Unique email address |
| `self_organization` | Organization | Personal organization |
| `tags` | Map<String, String?> | Custom metadata |
| `created_at` | DateTime | Creation timestamp |
| `updated_at` | DateTime | Last update timestamp |
| `deleted_at` | DateTime? | Soft delete timestamp |

### Organization

| Field | Type | Description |
|-------|------|-------------|
| `organization_id` | UUID | Unique identifier |
| `organization_type` | OrganizationType | User or Normal org |
| `name` | String? | Organization name (for Normal type) |
| `tags` | Map<String, String?> | Custom metadata |
| `created_at` | DateTime | Creation timestamp |
| `updated_at` | DateTime | Last update timestamp |
| `deleted_at` | DateTime? | Soft delete timestamp |

---

## Pulsar Topics

### Command Topic (configurable via PULSAR_TOPIC)

Used by isok-api to send commands to isok-agent.

**Message Schema (Command):**
```json
{
  "id": "check-uuid",
  "kind": {
    "Add": {
      "check": { /* CheckOutput */ }
    }
  }
}
```
or
```json
{
  "id": "check-uuid",
  "kind": {
    "Remove": "check-uuid"
  }
}
```

### HTTP Results Topic (`http`)

Used by isok-agent to publish check results.

**Message Schema (CheckMessage):**
```json
{
  "check_id": "uuid",
  "agent_id": "agent-identifier",
  "timestamp": "2024-01-15T10:30:00+00:00",
  "latency": 150,
  "fields": {
    "status_code": 200
  }
}
```

### Aggregated HTTP Topic (`aggregated-http`)

Used by isok-offloader to publish aggregated results.

**Message Schema (AggregatedCheckMessage):**
```json
{
  "check_id": "uuid",
  "timestamp": "2024-01-15T10:30:00+00:00",
  "latency": 150,
  "status_codes": {
    "_200": 3,
    "_300": 0,
    "_400": 0,
    "_500": 0
  }
}
```

---

## Database Schema

### isok-api Database (Checks)

```sql
CREATE TABLE checks (
  check_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_id UUID NOT NULL,
  kind JSONB NOT NULL,
  max_latency INTERVAL NOT NULL,
  interval INTERVAL NOT NULL,
  region VARCHAR NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ
);
```

### isok-proxy Database (Users & Organizations)

```sql
CREATE TYPE organisation_type AS ENUM ('user', 'normal');
CREATE TYPE organisation_user_role AS ENUM ('owner', 'member');

CREATE TABLE organizations (
  organization_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  type organisation_type NOT NULL,
  name TEXT,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ
);

CREATE TABLE organizations_tags (
  organization_id UUID REFERENCES organizations,
  key TEXT NOT NULL,
  value TEXT,
  PRIMARY KEY (organization_id, key)
);

CREATE TABLE users (
  user_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT NOT NULL,
  password TEXT NOT NULL,
  email_address TEXT UNIQUE NOT NULL,
  self_organization UUID UNIQUE REFERENCES organizations,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ
);

CREATE TABLE users_tags (
  user_id UUID REFERENCES users,
  key TEXT NOT NULL,
  value TEXT,
  PRIMARY KEY (user_id, key)
);

CREATE TABLE users_organizations (
  user_id UUID REFERENCES users,
  organization_id UUID REFERENCES organizations,
  role organisation_user_role DEFAULT 'member',
  PRIMARY KEY (organization_id, user_id)
);
```

---

## Warp10 Metrics

The offloader writes the following metrics to Warp10:

### http.request_duration
- **Type**: Long (milliseconds)
- **Labels**:
  - `check-id`: UUID of the check
  - `agent-id`: Identifier of the agent

### http.request_status
- **Type**: Int (HTTP status code)
- **Labels**:
  - `check-id`: UUID of the check
  - `agent-id`: Identifier of the agent
