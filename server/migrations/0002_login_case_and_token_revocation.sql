-- 1. Logins are case-insensitive: store them trimmed + lowercase and enforce
--    uniqueness on lower(login). Existing case-duplicates (Ivan / ivan) keep the
--    oldest account's login; the others get a "_dupN" suffix so the migration
--    never fails on real data (the admin can rename them afterwards).
--    Two steps, because UNIQUE(login) is checked row by row during UPDATE.
CREATE TEMP TABLE login_fix ON COMMIT DROP AS
SELECT id,
       CASE WHEN rn = 1 THEN l ELSE l || '_dup' || rn END AS new_login
FROM (
    SELECT id,
           lower(btrim(login)) AS l,
           row_number() OVER (PARTITION BY lower(btrim(login)) ORDER BY created_at, id) AS rn
    FROM users
) d;

DELETE FROM login_fix f USING users u WHERE u.id = f.id AND u.login = f.new_login;

UPDATE users u SET login = '__tmp_' || u.id::text FROM login_fix f WHERE f.id = u.id;
UPDATE users u SET login = f.new_login FROM login_fix f WHERE f.id = u.id;

CREATE UNIQUE INDEX users_login_lower_idx ON users (lower(login));

-- 2. JWT revocation: tokens issued before the last password change/reset are rejected.
ALTER TABLE users ADD COLUMN password_changed_at TIMESTAMPTZ;

-- 3. Teacher's debtor list: students with debt, largest first.
CREATE INDEX users_debt_idx ON users (debt_minutes DESC) WHERE role = 'student' AND debt_minutes > 0;
