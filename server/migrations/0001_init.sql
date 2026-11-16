CREATE TABLE groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role TEXT NOT NULL CHECK (role IN ('student', 'teacher', 'komendant', 'admin')),
    login TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    full_name TEXT NOT NULL,
    group_id UUID REFERENCES groups(id) ON DELETE SET NULL,
    debt_minutes BIGINT NOT NULL DEFAULT 0 CHECK (debt_minutes >= 0),
    balance_minutes BIGINT NOT NULL DEFAULT 0 CHECK (balance_minutes >= 0),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX users_group_idx ON users(group_id);
CREATE INDEX users_role_idx ON users(role);

CREATE TABLE push_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX push_tokens_user_idx ON push_tokens(user_id);

CREATE TABLE assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_by UUID NOT NULL REFERENCES users(id),
    minutes BIGINT NOT NULL CHECK (minutes > 0),
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX assignments_student_idx ON assignments(student_id);
CREATE INDEX assignments_created_idx ON assignments(created_at);

CREATE TABLE work_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES users(id),
    komendant_id UUID NOT NULL REFERENCES users(id),
    difficulty TEXT NOT NULL CHECK (difficulty IN ('light', 'hard')),
    coefficient INT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (status IN (
        'active', 'pending_confirmation', 'credited', 'expired', 'komendant_rejected')),
    raw_seconds BIGINT,
    rounded_minutes BIGINT,
    credited_minutes BIGINT,
    applied_to_debt BIGINT,
    applied_to_balance BIGINT,
    reverted_from_balance BIGINT,
    reverted_to_debt BIGINT,
    photo_path TEXT,
    photo_review TEXT CHECK (photo_review IN ('pending', 'approved', 'rejected')),
    photo_review_comment TEXT,
    photo_reviewed_by UUID REFERENCES users(id),
    photo_reviewed_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    rejected_reason TEXT,
    expired_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- at most one open session per student
CREATE UNIQUE INDEX work_sessions_one_open ON work_sessions(student_id)
    WHERE status IN ('active', 'pending_confirmation');
CREATE INDEX work_sessions_komendant_idx ON work_sessions(komendant_id);
CREATE INDEX work_sessions_ended_idx ON work_sessions(ended_at);
CREATE INDEX work_sessions_status_idx ON work_sessions(status);

CREATE TABLE rewards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    price_minutes BIGINT NOT NULL CHECK (price_minutes >= 0),
    stock BIGINT CHECK (stock IS NULL OR stock >= 0),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    image_path TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    reward_id UUID NOT NULL REFERENCES rewards(id),
    price_minutes BIGINT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'issued', 'cancelled')),
    issued_by UUID REFERENCES users(id),
    issued_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX orders_user_idx ON orders(user_id);
CREATE INDEX orders_status_idx ON orders(status);

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    data JSONB NOT NULL DEFAULT '{}',
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX notifications_user_idx ON notifications(user_id, read_at);

CREATE TABLE audit_log (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_id UUID,
    actor_role TEXT,
    action TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    details JSONB NOT NULL DEFAULT '{}',
    ip TEXT
);
CREATE INDEX audit_log_ts_idx ON audit_log(ts);
CREATE INDEX audit_log_action_idx ON audit_log(action);

CREATE TABLE komendant_log (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    komendant_id UUID NOT NULL,
    action TEXT NOT NULL,
    student_id UUID,
    session_id UUID,
    details JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX komendant_log_komendant_idx ON komendant_log(komendant_id, ts);

CREATE TABLE monthly_summaries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    year INT NOT NULL,
    month INT NOT NULL CHECK (month BETWEEN 1 AND 12),
    generated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (year, month)
);

CREATE TABLE summary_rows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    summary_id UUID NOT NULL REFERENCES monthly_summaries(id) ON DELETE CASCADE,
    student_id UUID NOT NULL,
    full_name TEXT NOT NULL,
    group_name TEXT,
    sessions_count BIGINT NOT NULL,
    rounded_minutes BIGINT NOT NULL,
    credited_minutes BIGINT NOT NULL,
    assigned_minutes BIGINT NOT NULL,
    debt_minutes BIGINT NOT NULL,
    balance_minutes BIGINT NOT NULL
);
CREATE INDEX summary_rows_summary_idx ON summary_rows(summary_id);
