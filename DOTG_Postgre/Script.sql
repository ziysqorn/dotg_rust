-- Database: dotg

-- DROP DATABASE IF EXISTS dotg;

CREATE DATABASE dotg
    WITH
    OWNER = postgres
    ENCODING = 'UTF8'
    LC_COLLATE = 'Vietnamese_Vietnam.1258'
    LC_CTYPE = 'Vietnamese_Vietnam.1258'
    LOCALE_PROVIDER = 'libc'
    TABLESPACE = pg_default
    CONNECTION LIMIT = -1
    IS_TEMPLATE = False;
---------------------------------------------------
create table users (
username varchar(12) primary key,
user_password varchar(20),
status bool
)
create table friends (
player1 varchar(12),
player2 varchar(12),
foreign key (player1) references users(username),
foreign key (player2) references users(username)
)
create table FriendRequests (
sender varchar(12),
receiver varchar(12),
foreign key (sender) references users(username),
foreign key (receiver) references users(username)
)
delete from users
delete from friends

select * from users
select * from FriendRequests
select * from friends
update users set status = false where username = 'haha'
delete from FriendRequests where sender = 'haha' and receiver = 'keke' 
or sender = 'keke' and receiver = 'haha'
delete from friends where player1 = 'haha' and player2 = 'keke' 
or player1 = 'keke' and player2 = 'haha'
----------------------------------------------------
GRANT SELECT, INSERT, UPDATE, DELETE 
ON ALL TABLES IN SCHEMA public 
TO app_user;