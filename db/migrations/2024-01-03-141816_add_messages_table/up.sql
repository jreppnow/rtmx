CREATE TABLE messages (
    id SERIAL, 
    sender VARCHAR(64) NOT NULL, 
    receiver VARCHAR(64) NOT NULL, 
    content VARCHAR(1024) NOT NULL,
    sent_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, 
    read_at TIMESTAMP NULL DEFAULT NULL,
    PRIMARY KEY (id)
);
