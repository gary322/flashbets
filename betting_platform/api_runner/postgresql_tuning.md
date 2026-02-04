# PostgreSQL Tuning for High Load (2000+ Concurrent Users)

## Connection Pool Optimization

### Application-side (Deadpool) Settings
```bash
# Environment variables
export EXPECTED_CONCURRENT_USERS=2500
export DB_MAX_CONNECTIONS=200      # Override if needed
export DB_MIN_CONNECTIONS=50       # Minimum idle connections
```

### PostgreSQL Configuration (postgresql.conf)

```conf
# Connection Settings
max_connections = 300              # Allow 200 for app + overhead
superuser_reserved_connections = 5 # Reserve for admin

# Memory Settings (for 16GB RAM server)
shared_buffers = 4GB              # 25% of RAM
effective_cache_size = 12GB       # 75% of RAM
work_mem = 16MB                   # RAM / max_connections / 2
maintenance_work_mem = 1GB        # For vacuum, index creation

# Checkpoint Settings
checkpoint_timeout = 15min
checkpoint_completion_target = 0.9
wal_buffers = 16MB
max_wal_size = 4GB
min_wal_size = 1GB

# Query Planner
random_page_cost = 1.1            # For SSD storage
effective_io_concurrency = 200    # For SSD storage

# Logging (for monitoring)
log_min_duration_statement = 1000  # Log queries > 1 second
log_connections = on
log_disconnections = on
log_lock_waits = on
deadlock_timeout = 1s

# Connection Pooling Mode
# Consider using PgBouncer for additional pooling
```

## PgBouncer Configuration (Optional but Recommended)

```ini
[databases]
betting_platform = host=localhost port=5432 dbname=betting_platform

[pgbouncer]
listen_port = 6432
listen_addr = *
auth_type = md5
pool_mode = transaction
max_client_conn = 3000
default_pool_size = 25
min_pool_size = 10
reserve_pool_size = 5
server_lifetime = 3600
server_idle_timeout = 600
```

## Monitoring Queries

### Check Current Connections
```sql
SELECT count(*) as total,
       state,
       usename,
       application_name
FROM pg_stat_activity
GROUP BY state, usename, application_name
ORDER BY total DESC;
```

### Check Pool Efficiency
```sql
-- Long running queries
SELECT pid, 
       now() - pg_stat_activity.query_start AS duration, 
       query, 
       state
FROM pg_stat_activity
WHERE (now() - pg_stat_activity.query_start) > interval '5 minutes';

-- Connection wait times
SELECT wait_event_type, 
       wait_event, 
       count(*) 
FROM pg_stat_activity 
WHERE wait_event IS NOT NULL 
GROUP BY wait_event_type, wait_event 
ORDER BY count DESC;
```

## Load Testing Recommendations

1. **Use pgbench for connection testing**:
```bash
pgbench -i -s 100 betting_platform
pgbench -c 200 -j 4 -T 300 betting_platform
```

2. **Monitor during load**:
```bash
# Watch active connections
watch -n 1 'psql -c "SELECT count(*) FROM pg_stat_activity"'

# Monitor locks
watch -n 1 'psql -c "SELECT * FROM pg_locks WHERE NOT granted"'
```

## Optimization Results

With these settings, the system should handle:
- 2000+ concurrent users
- 200 active database connections
- Sub-100ms query response times
- Automatic connection recycling
- Graceful degradation under extreme load

## Additional Recommendations

1. **Use Read Replicas** for read-heavy workloads
2. **Implement Query Caching** at application level
3. **Use Prepared Statements** to reduce parsing overhead
4. **Monitor with pg_stat_statements** extension
5. **Set up alerts** for connection pool exhaustion