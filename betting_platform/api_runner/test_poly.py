import requests
import json

url = "https://gamma-api.polymarket.com/markets?limit=1&active=true"
print(f"Fetching from: {url}")

response = requests.get(url)
print(f"Status: {response.status_code}")
print(f"Response length: {len(response.text)}")
print(f"First 200 chars: {response.text[:200]}")

try:
    data = response.json()
    print(f"\nParsed successfully\!")
    print(f"Type: {type(data)}")
    if isinstance(data, list):
        print(f"List length: {len(data)}")
        if data:
            print(f"First item keys: {list(data[0].keys())[:10]}")
except Exception as e:
    print(f"\nFailed to parse: {e}")
