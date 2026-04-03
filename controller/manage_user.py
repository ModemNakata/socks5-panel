#!/usr/bin/env python3
import argparse
import os
import sys
import requests

UNIT_VARIABLES_PATH = os.path.join(os.path.dirname(__file__), ".unit_variables")

def load_secret():
    if not os.path.exists(UNIT_VARIABLES_PATH):
        print(f"Error: {UNIT_VARIABLES_PATH} not found", file=sys.stderr)
        sys.exit(1)
    
    with open(UNIT_VARIABLES_PATH) as f:
        for line in f:
            line = line.strip()
            if line.startswith("SERVICE_2_SERVICE_PRE_SHARED_SECRET_KEY="):
                return line.split("=", 1)[1]
    
    print("Error: SERVICE_2_SERVICE_PRE_SHARED_SECRET_KEY not found in .unit_variables", file=sys.stderr)
    sys.exit(1)

def add_user(url, username, password, secret):
    response = requests.post(
        f"{url}/user",
        json={"username": username, "password": password},
        headers={"Authorization": f"Bearer {secret}"}
    )
    return response

def delete_user(url, username, secret):
    response = requests.delete(
        f"{url}/user",
        json={"username": username},
        headers={"Authorization": f"Bearer {secret}"}
    )
    return response

def main():
    parser = argparse.ArgumentParser(description="Manage users in Dante proxy passwd file")
    parser.add_argument("action", choices=["add", "remove"], help="Action to perform")
    parser.add_argument("url", help="Service URL (e.g., http://localhost:8698)")
    parser.add_argument("username", help="Username")
    parser.add_argument("password", nargs="?", help="Password (required for add action)")
    
    args = parser.parse_args()
    
    if args.action == "add" and not args.password:
        print("Error: password is required for add action", file=sys.stderr)
        sys.exit(1)
    
    secret = load_secret()
    
    if args.action == "add":
        response = add_user(args.url, args.username, args.password, secret)
    else:
        response = delete_user(args.url, args.username, secret)
    
    if response.status_code == 200:
        print(f"Success: {response.json()['message']}")
    else:
        print(f"Error: {response.status_code} - {response.text}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
