#!/usr/bin/env python3
"""
Polymarket Integration Test Suite
Tests all Polymarket endpoints and performs load testing
"""

import requests
import json
import time
import concurrent.futures
import random
from datetime import datetime

# Configuration
API_BASE = "http://localhost:8081/api"
WS_URL = "ws://localhost:8081/ws"

# Test results
results = {
    "total_requests": 0,
    "successful": 0,
    "failed": 0,
    "response_times": [],
    "errors": []
}

def test_endpoint(endpoint, method="GET", data=None, headers=None):
    """Test a single endpoint"""
    global results
    
    url = f"{API_BASE}/{endpoint}"
    start_time = time.time()
    
    try:
        if method == "GET":
            response = requests.get(url, headers=headers)
        elif method == "POST":
            response = requests.post(url, json=data, headers=headers)
        else:
            response = requests.delete(url, headers=headers)
        
        response_time = time.time() - start_time
        results["response_times"].append(response_time)
        results["total_requests"] += 1
        
        if response.status_code in [200, 201]:
            results["successful"] += 1
            return True, response.json() if response.text else {}
        else:
            results["failed"] += 1
            results["errors"].append(f"{endpoint}: {response.status_code}")
            return False, {"error": response.text}
    
    except Exception as e:
        results["failed"] += 1
        results["errors"].append(f"{endpoint}: {str(e)}")
        return False, {"error": str(e)}

def test_polymarket_markets():
    """Test Polymarket markets endpoint"""
    print("\n1. Testing Polymarket Markets...")
    success, data = test_endpoint("polymarket/markets")
    
    if success:
        print(f"   ✅ Markets endpoint working")
        if isinstance(data, dict) and "data" in data:
            print(f"   Found {len(data.get('data', []))} markets")
    else:
        print(f"   ❌ Markets endpoint failed: {data}")

def test_order_book():
    """Test order book endpoint"""
    print("\n2. Testing Order Book...")
    token_id = "12345"  # Test token ID
    success, data = test_endpoint(f"polymarket/orderbook/{token_id}")
    
    if success:
        print(f"   ✅ Order book endpoint working")
    else:
        print(f"   ⚠️  Order book endpoint returned: {data}")

def test_order_submission():
    """Test order submission"""
    print("\n3. Testing Order Submission...")
    
    order_data = {
        "order": {
            "salt": str(random.randint(1000000, 9999999)),
            "maker": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb4",
            "signer": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb4",
            "taker": "0x0000000000000000000000000000000000000000",
            "token_id": "test_token_123",
            "maker_amount": "100",
            "taker_amount": "50",
            "expiration": "9999999999",
            "nonce": str(random.randint(1, 1000)),
            "fee_rate_bps": "10",
            "side": 0,
            "signature_type": 0
        },
        "signature": "0x" + "00" * 65,  # Mock signature
        "market_id": "test_market"
    }
    
    success, data = test_endpoint("polymarket/orders/submit", "POST", order_data)
    
    if success:
        print(f"   ✅ Order submission working")
        print(f"   Order ID: {data.get('order_id', 'N/A')}")
    else:
        print(f"   ⚠️  Order submission returned: {data}")

def test_user_positions():
    """Test user positions endpoint"""
    print("\n4. Testing User Positions...")
    success, data = test_endpoint("polymarket/positions")
    
    if success:
        print(f"   ✅ Positions endpoint working")
    else:
        print(f"   ⚠️  Positions endpoint returned: {data}")

def test_balances():
    """Test balances endpoint"""
    print("\n5. Testing Balances...")
    success, data = test_endpoint("polymarket/balances")
    
    if success:
        print(f"   ✅ Balances endpoint working")
    else:
        print(f"   ⚠️  Balances endpoint returned: {data}")

def load_test_markets(num_requests=100):
    """Load test the markets endpoint"""
    print(f"\n6. Load Testing Markets Endpoint ({num_requests} requests)...")
    
    def fetch_markets():
        return test_endpoint("polymarket/markets")
    
    start_time = time.time()
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        futures = [executor.submit(fetch_markets) for _ in range(num_requests)]
        concurrent.futures.wait(futures)
    
    duration = time.time() - start_time
    rps = num_requests / duration
    
    print(f"   Completed {num_requests} requests in {duration:.2f}s")
    print(f"   Requests per second: {rps:.2f}")

def test_concurrent_orders(num_users=10, orders_per_user=5):
    """Test concurrent order submission"""
    print(f"\n7. Concurrent Order Test ({num_users} users, {orders_per_user} orders each)...")
    
    def submit_orders(user_id):
        for i in range(orders_per_user):
            order_data = {
                "order": {
                    "salt": str(random.randint(1000000, 9999999)),
                    "maker": f"0x{user_id:040x}",
                    "signer": f"0x{user_id:040x}",
                    "taker": "0x0000000000000000000000000000000000000000",
                    "token_id": f"token_{user_id}_{i}",
                    "maker_amount": str(random.randint(10, 1000)),
                    "taker_amount": str(random.randint(10, 1000)),
                    "expiration": "9999999999",
                    "nonce": str(i),
                    "fee_rate_bps": "10",
                    "side": random.choice([0, 1]),
                    "signature_type": 0
                },
                "signature": "0x" + "00" * 65,
                "market_id": f"market_{user_id}"
            }
            test_endpoint("polymarket/orders/submit", "POST", order_data)
    
    start_time = time.time()
    
    with concurrent.futures.ThreadPoolExecutor(max_workers=num_users) as executor:
        futures = [executor.submit(submit_orders, i) for i in range(num_users)]
        concurrent.futures.wait(futures)
    
    duration = time.time() - start_time
    total_orders = num_users * orders_per_user
    ops = total_orders / duration
    
    print(f"   Submitted {total_orders} orders in {duration:.2f}s")
    print(f"   Orders per second: {ops:.2f}")

def test_polymarket_health():
    """Test Polymarket health endpoint"""
    print("\n8. Testing Polymarket Health...")
    success, data = test_endpoint("polymarket/health")
    
    if success:
        print(f"   ✅ Health endpoint working")
        if isinstance(data, dict):
            print(f"   CLOB Connected: {data.get('clobConnected', 'N/A')}")
            print(f"   WebSocket Connected: {data.get('websocketConnected', 'N/A')}")
            print(f"   Database Connected: {data.get('databaseConnected', 'N/A')}")
    else:
        print(f"   ⚠️  Health endpoint returned: {data}")

def print_summary():
    """Print test summary"""
    print("\n" + "="*60)
    print("TEST SUMMARY")
    print("="*60)
    print(f"Total Requests: {results['total_requests']}")
    print(f"Successful: {results['successful']}")
    print(f"Failed: {results['failed']}")
    
    if results['response_times']:
        avg_time = sum(results['response_times']) / len(results['response_times'])
        min_time = min(results['response_times'])
        max_time = max(results['response_times'])
        print(f"\nResponse Times:")
        print(f"  Average: {avg_time*1000:.2f}ms")
        print(f"  Min: {min_time*1000:.2f}ms")
        print(f"  Max: {max_time*1000:.2f}ms")
    
    if results['errors']:
        print(f"\nErrors ({len(results['errors'])}):")
        for error in results['errors'][:5]:  # Show first 5 errors
            print(f"  - {error}")
    
    # Check Polymarket integration status
    print("\nPolymarket Integration Status:")
    if results['successful'] > 0:
        print("  ✅ API endpoints responding")
    else:
        print("  ❌ API endpoints not working")
    
    # Performance grade
    if results['response_times']:
        avg_ms = avg_time * 1000
        if avg_ms < 100:
            grade = "A+ (Excellent)"
        elif avg_ms < 200:
            grade = "A (Very Good)"
        elif avg_ms < 500:
            grade = "B (Good)"
        elif avg_ms < 1000:
            grade = "C (Acceptable)"
        else:
            grade = "D (Needs Improvement)"
        print(f"\nPerformance Grade: {grade}")

def main():
    print("="*60)
    print("POLYMARKET INTEGRATION TEST SUITE")
    print(f"Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("="*60)
    
    # Run tests
    test_polymarket_markets()
    test_order_book()
    test_order_submission()
    test_user_positions()
    test_balances()
    test_polymarket_health()
    
    # Load tests
    load_test_markets(50)
    test_concurrent_orders(5, 3)
    
    # Print summary
    print_summary()
    
    print("\n" + "="*60)
    print("TEST COMPLETE")
    print("="*60)

if __name__ == "__main__":
    main()