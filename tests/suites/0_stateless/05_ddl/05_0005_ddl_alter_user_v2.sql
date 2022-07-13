set enable_planner_v2=1;
DROP USER IF EXISTS 'test-e'@'localhost';
DROP USER IF EXISTS 'test-g'@'localhost';
DROP USER IF EXISTS 'test-h'@'localhost';
DROP USER IF EXISTS 'test-i@localhost';

CREATE USER 'test-e'@'localhost' IDENTIFIED BY 'password';
ALTER USER 'test-e'@'localhost' IDENTIFIED BY 'new-password';
ALTER USER 'test1'@'localhost' IDENTIFIED BY 'password'; -- {ErrorCode 2201}

CREATE USER 'test-g'@'localhost' IDENTIFIED WITH sha256_password BY 'password';
ALTER USER 'test-g'@'localhost' IDENTIFIED WITH sha256_password BY 'new-password';

CREATE USER 'test-h'@'localhost' IDENTIFIED WITH double_sha1_password BY 'password';
ALTER USER 'test-h'@'localhost' IDENTIFIED WITH double_sha1_password BY 'new-password';

CREATE USER 'test-i@localhost' IDENTIFIED WITH sha256_password BY 'password';
ALTER USER 'test-i@localhost' IDENTIFIED WITH sha256_password BY 'new-password';

DROP USER IF EXISTS 'test-e'@'localhost';
DROP USER IF EXISTS 'test-g'@'localhost';
DROP USER IF EXISTS 'test-h'@'localhost';
DROP USER IF EXISTS 'test-i@localhost';
