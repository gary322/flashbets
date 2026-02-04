-- PostgreSQL Database Setup Script for Betting Platform
-- Run this script to create the database and user

-- Create the database user
CREATE USER betting_user WITH PASSWORD 'betting_pass';

-- Create the database
CREATE DATABASE betting_platform OWNER betting_user;

-- Grant all privileges on the database to the user
GRANT ALL PRIVILEGES ON DATABASE betting_platform TO betting_user;

-- Connect to the betting_platform database
\c betting_platform;

-- Grant schema permissions
GRANT ALL ON SCHEMA public TO betting_user;

-- Instructions:
-- 1. Run as PostgreSQL superuser: sudo -u postgres psql < setup_database.sql
-- 2. Or connect to PostgreSQL and run: \i setup_database.sql
-- 3. The API will automatically create tables when it starts