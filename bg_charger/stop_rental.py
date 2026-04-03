#!/usr/bin/env python3
"""
Stop rental script.

This script:
1. Takes a rental_id as argument
2. Calls the proxy server API to remove user credentials
3. Only if API call succeeds, sets rental is_active to false
4. Handles errors properly - if API fails, rental stays active

Usage:
    python stop_rental.py <rental_id>
    
Example:
    python stop_rental.py 550e8400-e29b-41d4-a716-446655440000
"""

import os
import sys
import logging
from datetime import datetime, timezone
from dotenv import load_dotenv
import psycopg2
from psycopg2.extras import RealDictCursor
import requests
from requests.exceptions import RequestException, HTTPError, Timeout, ConnectionError

# Configure logging with DEBUG level for extensive output
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Load environment variables
env_path = os.path.join(os.path.dirname(__file__), '..', '.env')
logger.debug(f"Loading environment from: {env_path}")
load_dotenv(env_path)

DB_URL = os.getenv('DB_URL')
DOMAIN = os.getenv('DOMAIN')

logger.debug(f"DB_URL loaded: {'***' if DB_URL else 'NOT SET'}")
logger.debug(f"DOMAIN loaded: {DOMAIN if DOMAIN else 'NOT SET'}")

if not DB_URL:
    raise ValueError("DB_URL not found in .env file")
if not DOMAIN:
    raise ValueError("DOMAIN not found in .env file")


def get_rental(conn, rental_id: str) -> dict:
    """Fetch rental information from database."""
    logger.debug(f"[get_rental] Fetching rental for rental_id: {rental_id}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT id, user_id, server_id, username, password, is_active, created_at, updated_at
            FROM rental
            WHERE id = %s
        """
        logger.debug(f"[get_rental] Executing query with rental_id: {rental_id}")
        cur.execute(query, (rental_id,))
        rental = cur.fetchone()
        if rental:
            logger.debug(f"[get_rental] Found rental: id={rental['id']}, user_id={rental['user_id']}, server_id={rental['server_id']}, username={rental['username']}, is_active={rental['is_active']}")
            return rental
        logger.error(f"[get_rental] Rental not found for rental_id: {rental_id}")
        return None


def get_proxy_server(conn, server_id: str) -> dict:
    """Fetch proxy server information from database."""
    logger.debug(f"[get_proxy_server] Fetching server for server_id: {server_id}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT id, country, codename, controller_key, price, speed, slots_available, 
                   proxies_rented, port, is_promo, is_ready, created_at
            FROM proxy_server
            WHERE id = %s
        """
        logger.debug(f"[get_proxy_server] Executing query with server_id: {server_id}")
        cur.execute(query, (server_id,))
        server = cur.fetchone()
        if server:
            logger.debug(f"[get_proxy_server] Found server: id={server['id']}, codename={server['codename']}, controller_key=***, port={server['port']}")
            return server
        logger.error(f"[get_proxy_server] Proxy server not found for server_id: {server_id}")
        return None


def remove_user_from_proxy(codename: str, controller_key: str, username: str) -> bool:
    """
    Remove user credentials from proxy server via API call.
    
    Returns True if successful, False otherwise.
    """
    controller_url = f"https://{codename}.{DOMAIN}/api"
    endpoint = f"{controller_url}/user"
    
    logger.info(f"[remove_user_from_proxy] Removing user '{username}' from proxy server '{codename}'")
    logger.debug(f"[remove_user_from_proxy] API endpoint: {endpoint}")
    logger.debug(f"[remove_user_from_proxy] Using controller_key: ***")
    
    headers = {
        'Authorization': f'Bearer {controller_key}',
        'Content-Type': 'application/json'
    }
    payload = {
        'username': username
    }
    
    logger.debug(f"[remove_user_from_proxy] Sending DELETE request with payload: {payload}")
    
    try:
        response = requests.delete(
            endpoint,
            json=payload,
            headers=headers,
            timeout=30  # 30 second timeout
        )
        
        logger.debug(f"[remove_user_from_proxy] Response status code: {response.status_code}")
        logger.debug(f"[remove_user_from_proxy] Response body: {response.text}")
        
        # Raise HTTPError for bad status codes (4xx, 5xx)
        response.raise_for_status()
        
        logger.info(f"[remove_user_from_proxy] Successfully removed user '{username}' from proxy server '{codename}'")
        return True
        
    except HTTPError as e:
        logger.error(f"[remove_user_from_proxy] HTTP error: {e}")
        logger.error(f"[remove_user_from_proxy] Response status: {e.response.status_code if e.response else 'N/A'}")
        logger.error(f"[remove_user_from_proxy] Response body: {e.response.text if e.response else 'N/A'}")
        return False
    except Timeout as e:
        logger.error(f"[remove_user_from_proxy] Request timeout: {e}")
        return False
    except ConnectionError as e:
        logger.error(f"[remove_user_from_proxy] Connection error: {e}")
        return False
    except RequestException as e:
        logger.error(f"[remove_user_from_proxy] Request exception: {e}")
        return False
    except Exception as e:
        logger.error(f"[remove_user_from_proxy] Unexpected error: {e}", exc_info=True)
        return False


def deactivate_rental(conn, rental_id: str):
    """Set rental is_active to false in database."""
    logger.debug(f"[deactivate_rental] Deactivating rental_id: {rental_id}")
    with conn.cursor() as cur:
        query = """
            UPDATE rental
            SET is_active = false, updated_at = %s
            WHERE id = %s
        """
        now = datetime.now(timezone.utc)
        logger.debug(f"[deactivate_rental] Executing query with now: {now}, rental_id: {rental_id}")
        cur.execute(query, (now, rental_id))
        logger.debug(f"[deactivate_rental] Rental {rental_id} deactivated successfully")


def update_server_slots(conn, server_id: str, increment: bool = True):
    """
    Update proxy server slots after rental stops.
    
    Args:
        conn: Database connection
        server_id: Server UUID
        increment: If True, increment slots_available and decrement proxies_rented
    """
    logger.debug(f"[update_server_slots] Updating slots for server_id: {server_id}, increment: {increment}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        if increment:
            query = """
                UPDATE proxy_server
                SET slots_available = slots_available + 1,
                    proxies_rented = proxies_rented - 1
                WHERE id = %s
                RETURNING slots_available, proxies_rented
            """
        else:
            query = """
                UPDATE proxy_server
                SET slots_available = slots_available - 1,
                    proxies_rented = proxies_rented + 1
                WHERE id = %s
                RETURNING slots_available, proxies_rented
            """
        logger.debug(f"[update_server_slots] Executing query with server_id: {server_id}")
        cur.execute(query, (server_id,))
        result = cur.fetchone()
        if result:
            logger.debug(f"[update_server_slots] Server slots updated: slots_available={result['slots_available']}, proxies_rented={result['proxies_rented']}")
        else:
            logger.warning(f"[update_server_slots] Server {server_id} not found for slot update")


def stop_rental(rental_id: str) -> dict:
    """
    Main function to stop a rental.
    
    Returns a dict with the result of the operation.
    """
    logger.info("=" * 60)
    logger.info(f"Stopping rental: {rental_id}")
    logger.info("=" * 60)
    
    conn = None
    try:
        logger.debug("Connecting to database...")
        conn = psycopg2.connect(DB_URL)
        logger.info("Database connection established")
        
        # Get rental information
        logger.debug(f"Fetching rental information for: {rental_id}")
        rental = get_rental(conn, rental_id)
        
        if not rental:
            error_msg = f"Rental {rental_id} not found"
            logger.error(error_msg)
            return {
                'success': False,
                'error': error_msg,
                'rental_id': rental_id
            }
        
        if not rental['is_active']:
            logger.info(f"Rental {rental_id} is already inactive")
            return {
                'success': True,
                'message': 'Rental already inactive',
                'rental_id': rental_id
            }
        
        # Get proxy server information
        server_id = str(rental['server_id'])
        logger.debug(f"Fetching proxy server information for: {server_id}")
        server = get_proxy_server(conn, server_id)
        
        if not server:
            error_msg = f"Proxy server {server_id} not found"
            logger.error(error_msg)
            return {
                'success': False,
                'error': error_msg,
                'rental_id': rental_id,
                'server_id': server_id
            }
        
        # Remove user from proxy server
        codename = server['codename']
        controller_key = server['controller_key']
        username = rental['username']
        
        api_success = remove_user_from_proxy(codename, controller_key, username)
        
        if not api_success:
            error_msg = f"Failed to remove user '{username}' from proxy server '{codename}' - API call failed"
            logger.error(error_msg)
            logger.error(f"Rental {rental_id} will remain active and will be retried on next run")
            return {
                'success': False,
                'error': error_msg,
                'rental_id': rental_id,
                'server_id': server_id,
                'username': username,
                'api_call_failed': True
            }
        
        # API call succeeded - now update database
        logger.info("API call succeeded, updating database...")
        
        # Deactivate rental
        deactivate_rental(conn, rental_id)
        
        # Update server slots
        update_server_slots(conn, server_id, increment=True)
        
        # Commit changes
        logger.debug("Committing database transaction...")
        conn.commit()
        logger.info("Database transaction committed")
        
        logger.info("=" * 60)
        logger.info(f"Rental {rental_id} stopped successfully")
        logger.info(f"  - User '{username}' removed from proxy '{codename}'")
        logger.info(f"  - Rental deactivated")
        logger.info(f"  - Server slots updated")
        logger.info("=" * 60)
        
        return {
            'success': True,
            'rental_id': rental_id,
            'server_id': server_id,
            'username': username,
            'message': 'Rental stopped successfully'
        }
        
    except Exception as e:
        logger.error(f"Error stopping rental {rental_id}: {e}", exc_info=True)
        if conn:
            logger.debug("Rolling back database transaction...")
            conn.rollback()
            logger.debug("Database transaction rolled back")
        return {
            'success': False,
            'error': str(e),
            'rental_id': rental_id
        }
    finally:
        if conn:
            logger.debug("Closing database connection")
            conn.close()
            logger.debug("Database connection closed")


def main():
    """Main entry point."""
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <rental_id>")
        print(f"Example: {sys.argv[0]} 550e8400-e29b-41d4-a716-446655440000")
        sys.exit(1)
    
    rental_id = sys.argv[1]
    logger.info(f"Script called with rental_id: {rental_id}")
    
    result = stop_rental(rental_id)
    
    if result['success']:
        logger.info(f"Operation completed successfully: {result.get('message', '')}")
        sys.exit(0)
    else:
        logger.error(f"Operation failed: {result.get('error', 'Unknown error')}")
        sys.exit(1)


if __name__ == '__main__':
    main()
