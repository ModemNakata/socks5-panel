#!/usr/bin/env python3
"""
Hourly billing script for rental charges.

This script:
1. Fetches all active rentals (is_active = true)
2. For each rental, calculates hourly rate: server_price / (29 * 24)
3. Deducts the hourly rate from user's balance
4. Updates the rental's updated_at timestamp (for logging purposes)

Should be run as a cron job every hour (e.g., at minute 0).
"""

import os
import logging
from datetime import datetime, timezone
from decimal import Decimal, ROUND_HALF_UP
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

# Constants for hourly rate calculation
BILLING_PERIOD_DAYS = Decimal('29')
HOURS_PER_DAY = Decimal('24')
HOURS_IN_BILLING_PERIOD = BILLING_PERIOD_DAYS * HOURS_PER_DAY  # 696 hours
logger.debug(f"Billing period: {BILLING_PERIOD_DAYS} days = {HOURS_IN_BILLING_PERIOD} hours")


def calculate_hourly_rate(server_price: Decimal) -> Decimal:
    """
    Calculate hourly rate from server price.
    
    Formula: server_price / (29 days * 24 hours)
    Example: 1.00 EUR / 696 = 0.00143678161 EUR/hour
    """
    logger.debug(f"[calculate_hourly_rate] Input server_price: {server_price}")
    hourly_rate = server_price / HOURS_IN_BILLING_PERIOD
    logger.debug(f"[calculate_hourly_rate] Raw hourly_rate: {hourly_rate}")
    # Round to 11 decimal places for precision
    rounded_rate = hourly_rate.quantize(Decimal('0.00000000001'), rounding=ROUND_HALF_UP)
    logger.debug(f"[calculate_hourly_rate] Rounded hourly_rate: {rounded_rate}")
    return rounded_rate


def get_active_rentals(conn):
    """Fetch all active rentals."""
    logger.debug("[get_active_rentals] Starting query for active rentals")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT id, user_id, server_id, username, password, updated_at
            FROM rental
            WHERE is_active = true
        """
        logger.debug(f"[get_active_rentals] Executing query: {query.strip()}")
        cur.execute(query)
        rentals = cur.fetchall()
        logger.debug(f"[get_active_rentals] Found {len(rentals)} active rentals")
        for i, rental in enumerate(rentals):
            logger.debug(f"[get_active_rentals] Rental {i+1}: id={rental['id']}, user_id={rental['user_id']}, server_id={rental['server_id']}, username={rental['username']}, updated_at={rental['updated_at']}")
        return rentals


def get_user_balance(conn, user_id: str) -> Decimal:
    """Fetch user's current balance from user_session table."""
    logger.debug(f"[get_user_balance] Fetching balance for user_id: {user_id}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT balance
            FROM user_session
            WHERE id = %s
        """
        logger.debug(f"[get_user_balance] Executing query with user_id: {user_id}")
        cur.execute(query, (user_id,))
        row = cur.fetchone()
        if row:
            balance = Decimal(row['balance'])
            logger.debug(f"[get_user_balance] Found balance: {balance} for user_id: {user_id}")
            return balance
        logger.error(f"[get_user_balance] User session not found for user_id: {user_id}")
        raise ValueError(f"User session not found for user_id: {user_id}")


def get_server_price(conn, server_id: str) -> Decimal:
    """Fetch server's price from proxy_server table."""
    logger.debug(f"[get_server_price] Fetching price for server_id: {server_id}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT price
            FROM proxy_server
            WHERE id = %s
        """
        logger.debug(f"[get_server_price] Executing query with server_id: {server_id}")
        cur.execute(query, (server_id,))
        row = cur.fetchone()
        if row:
            price = Decimal(row['price'])
            logger.debug(f"[get_server_price] Found price: {price} for server_id: {server_id}")
            return price
        logger.error(f"[get_server_price] Proxy server not found for server_id: {server_id}")
        raise ValueError(f"Proxy server not found for server_id: {server_id}")


def update_user_balance(conn, user_id: str, new_balance: Decimal):
    """Update user's balance in user_session table."""
    logger.debug(f"[update_user_balance] Updating balance for user_id: {user_id} to: {new_balance}")
    with conn.cursor() as cur:
        query = """
            UPDATE user_session
            SET balance = %s
            WHERE id = %s
        """
        logger.debug(f"[update_user_balance] Executing query with new_balance: {new_balance}, user_id: {user_id}")
        cur.execute(query, (str(new_balance), user_id))
        logger.debug(f"[update_user_balance] Balance updated successfully for user_id: {user_id}")


def update_rental_timestamp(conn, rental_id: str):
    """Update rental's updated_at timestamp to now (for logging purposes)."""
    now = datetime.now(timezone.utc)
    logger.debug(f"[update_rental_timestamp] Updating rental_id: {rental_id} updated_at to: {now}")
    with conn.cursor() as cur:
        query = """
            UPDATE rental
            SET updated_at = %s
            WHERE id = %s
        """
        logger.debug(f"[update_rental_timestamp] Executing query with now: {now}, rental_id: {rental_id}")
        cur.execute(query, (now, rental_id))
        logger.debug(f"[update_rental_timestamp] Timestamp updated successfully for rental_id: {rental_id}")


def remove_user_from_proxy_api(codename: str, controller_key: str, username: str) -> bool:
    """
    Remove user credentials from proxy server via API call.
    
    Returns True if successful, False otherwise.
    """
    controller_url = f"https://{codename}.{DOMAIN}/api"
    endpoint = f"{controller_url}/user"
    
    logger.info(f"[remove_user_from_proxy_api] Removing user '{username}' from proxy server '{codename}'")
    logger.debug(f"[remove_user_from_proxy_api] API endpoint: {endpoint}")
    
    headers = {
        'Authorization': f'Bearer {controller_key}',
        'Content-Type': 'application/json'
    }
    payload = {
        'username': username
    }
    
    logger.debug(f"[remove_user_from_proxy_api] Sending DELETE request with payload: {payload}")
    
    try:
        response = requests.delete(
            endpoint,
            json=payload,
            headers=headers,
            timeout=30  # 30 second timeout
        )
        
        logger.debug(f"[remove_user_from_proxy_api] Response status code: {response.status_code}")
        logger.debug(f"[remove_user_from_proxy_api] Response body: {response.text[:500] if response.text else 'empty'}")
        
        # Raise HTTPError for bad status codes (4xx, 5xx)
        response.raise_for_status()
        
        logger.info(f"[remove_user_from_proxy_api] Successfully removed user '{username}' from proxy server '{codename}'")
        return True
        
    except HTTPError as e:
        logger.error(f"[remove_user_from_proxy_api] HTTP error: {e}")
        logger.error(f"[remove_user_from_proxy_api] Response status: {e.response.status_code if e.response else 'N/A'}")
        logger.error(f"[remove_user_from_proxy_api] Response body: {e.response.text[:500] if e.response else 'N/A'}")
        return False
    except Timeout as e:
        logger.error(f"[remove_user_from_proxy_api] Request timeout: {e}")
        return False
    except ConnectionError as e:
        logger.error(f"[remove_user_from_proxy_api] Connection error: {e}")
        return False
    except RequestException as e:
        logger.error(f"[remove_user_from_proxy_api] Request exception: {e}")
        return False
    except Exception as e:
        logger.error(f"[remove_user_from_proxy_api] Unexpected error: {e}", exc_info=True)
        return False


def get_proxy_server_info(conn, server_id: str) -> dict:
    """Fetch proxy server information from database."""
    logger.debug(f"[get_proxy_server_info] Fetching server for server_id: {server_id}")
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT id, codename, controller_key, port
            FROM proxy_server
            WHERE id = %s
        """
        logger.debug(f"[get_proxy_server_info] Executing query with server_id: {server_id}")
        cur.execute(query, (server_id,))
        server = cur.fetchone()
        if server:
            logger.debug(f"[get_proxy_server_info] Found server: id={server['id']}, codename={server['codename']}, port={server['port']}")
            return server
        logger.error(f"[get_proxy_server_info] Proxy server not found for server_id: {server_id}")
        return None


def stop_rental_for_negative_balance(conn, rental_id: str, user_id: str, server_id: str, username: str) -> dict:
    """
    Stop a rental due to negative balance.
    
    1. Set user balance to 0
    2. Call proxy API to remove user
    3. Only if API succeeds, deactivate rental and update server slots
    
    Returns dict with result information.
    """
    logger.info(f"[stop_rental_for_negative_balance] Stopping rental {rental_id} due to negative balance")
    
    # Step 1: Set user balance to 0
    logger.debug(f"[stop_rental_for_negative_balance] Setting user {user_id} balance to 0")
    update_user_balance(conn, user_id, Decimal('0'))
    logger.info(f"[stop_rental_for_negative_balance] User {user_id} balance set to 0")
    
    # Step 2: Get proxy server info
    server = get_proxy_server_info(conn, server_id)
    if not server:
        logger.error(f"[stop_rental_for_negative_balance] Proxy server {server_id} not found")
        return {
            'success': False,
            'error': 'Proxy server not found',
            'balance_set_to_zero': True,
            'rental_deactivated': False
        }
    
    # Step 3: Call proxy API to remove user
    api_success = remove_user_from_proxy_api(
        codename=server['codename'],
        controller_key=server['controller_key'],
        username=username
    )
    
    if not api_success:
        logger.error(f"[stop_rental_for_negative_balance] API call failed for rental {rental_id}")
        logger.error(f"[stop_rental_for_negative_balance] Rental will remain active, will retry on next billing cycle")
        return {
            'success': False,
            'error': 'Proxy API call failed',
            'balance_set_to_zero': True,
            'rental_deactivated': False,
            'api_failed': True
        }
    
    # Step 4: API succeeded - deactivate rental and update server slots
    logger.debug(f"[stop_rental_for_negative_balance] Deactivating rental {rental_id}")
    with conn.cursor() as cur:
        # Deactivate rental
        cur.execute("""
            UPDATE rental
            SET is_active = false, updated_at = %s
            WHERE id = %s
        """, (datetime.now(timezone.utc), rental_id))
        logger.debug(f"[stop_rental_for_negative_balance] Rental {rental_id} deactivated")
        
        # Update server slots
        cur.execute("""
            UPDATE proxy_server
            SET slots_available = slots_available + 1,
                proxies_rented = proxies_rented - 1
            WHERE id = %s
        """, (server_id,))
        logger.debug(f"[stop_rental_for_negative_balance] Server {server_id} slots updated")
    
    logger.info(f"[stop_rental_for_negative_balance] Rental {rental_id} stopped successfully")
    return {
        'success': True,
        'balance_set_to_zero': True,
        'rental_deactivated': True,
        'user_removed_from_proxy': True
    }


def process_rental(conn, rental) -> dict:
    """
    Process a single rental for billing.

    Returns a dict with billing result information.
    """
    rental_id = str(rental['id'])
    user_id = str(rental['user_id'])
    server_id = str(rental['server_id'])
    username = rental['username']

    logger.info(f"[process_rental] === Processing rental: {rental_id} ===")
    logger.debug(f"[process_rental] user_id: {user_id}, server_id: {server_id}, username: {username}")
    
    # Get server price and calculate hourly rate
    logger.debug(f"[process_rental] Fetching server price for server_id: {server_id}")
    server_price = get_server_price(conn, server_id)
    logger.debug(f"[process_rental] Server price: {server_price}")
    
    hourly_rate = calculate_hourly_rate(server_price)
    logger.debug(f"[process_rental] Calculated hourly rate: {hourly_rate}")
    
    # Get user balance
    logger.debug(f"[process_rental] Fetching user balance for user_id: {user_id}")
    user_balance = get_user_balance(conn, user_id)
    logger.debug(f"[process_rental] User balance: {user_balance}")
    
    # Calculate new balance
    new_balance = user_balance - hourly_rate
    logger.debug(f"[process_rental] New balance calculation: {user_balance} - {hourly_rate} = {new_balance}")
    
    # Check if new balance would be negative
    if new_balance < 0:
        logger.warning(f"[process_rental] Rental {rental_id}: NEGATIVE BALANCE DETECTED!")
        logger.warning(f"[process_rental] User balance: {user_balance}, Hourly rate: {hourly_rate}, New balance would be: {new_balance}")
        logger.info(f"[process_rental] Initiating rental stop due to insufficient balance")
        
        # Stop rental due to negative balance
        stop_result = stop_rental_for_negative_balance(
            conn, rental_id, user_id, server_id, username
        )
        
        if stop_result['success']:
            logger.info(f"[process_rental] Rental {rental_id} stopped successfully due to negative balance")
            return {
                'rental_id': rental_id,
                'user_id': user_id,
                'server_id': server_id,
                'status': 'stopped_negative_balance',
                'reason': 'insufficient_balance',
                'old_balance': str(user_balance),
                'hourly_rate': str(hourly_rate),
                'balance_set_to_zero': True,
                'rental_deactivated': True
            }
        else:
            # API failed - balance was set to 0 but rental stays active
            logger.warning(f"[process_rental] Rental {rental_id}: API failed, rental remains active")
            logger.warning(f"[process_rental] Will retry on next billing cycle")
            return {
                'rental_id': rental_id,
                'user_id': user_id,
                'server_id': server_id,
                'status': 'balance_zero_api_failed',
                'reason': 'insufficient_balance_api_error',
                'old_balance': str(user_balance),
                'balance_set_to_zero': True,
                'rental_deactivated': False,
                'error': stop_result.get('error', 'Unknown error')
            }
    
    # Normal billing flow - balance is sufficient
    logger.debug(f"[process_rental] Balance sufficient, proceeding with normal billing")
    
    # Update database
    logger.debug(f"[process_rental] Updating user balance in database")
    update_user_balance(conn, user_id, new_balance)
    
    logger.debug(f"[process_rental] Updating rental timestamp in database")
    update_rental_timestamp(conn, rental_id)
    
    logger.info(
        f"[process_rental] Rental {rental_id}: CHARGED {hourly_rate} EUR "
        f"(server_price={server_price}, old_balance={user_balance}, new_balance={new_balance})"
    )
    
    return {
        'rental_id': rental_id,
        'user_id': user_id,
        'server_id': server_id,
        'status': 'billed',
        'server_price': str(server_price),
        'hourly_rate': str(hourly_rate),
        'old_balance': str(user_balance),
        'new_balance': str(new_balance)
    }


def main():
    """Main entry point for hourly billing."""
    logger.info("=" * 60)
    logger.info("Starting hourly billing process")
    logger.info("=" * 60)
    
    if not DB_URL:
        logger.error("DB_URL is not configured")
        return
    
    logger.debug(f"DB_URL configured: {'***' if DB_URL else 'NOT SET'}")
    
    conn = None
    try:
        logger.debug("Attempting to connect to database...")
        conn = psycopg2.connect(DB_URL)
        logger.info("Database connection established successfully")
        
        # Get all active rentals
        logger.debug("Fetching all active rentals...")
        rentals = get_active_rentals(conn)
        logger.info(f"Found {len(rentals)} active rentals to process")
        
        if len(rentals) == 0:
            logger.info("No active rentals found. Exiting.")
            return
        
        results = []
        for i, rental in enumerate(rentals):
            logger.info(f"Processing rental {i+1}/{len(rentals)}: {rental['id']}")
            try:
                result = process_rental(conn, rental)
                results.append(result)
                logger.debug(f"Rental {rental['id']} processed with status: {result['status']}")
            except Exception as e:
                logger.error(f"Error processing rental {rental['id']}: {e}", exc_info=True)
                results.append({
                    'rental_id': str(rental['id']),
                    'status': 'error',
                    'error': str(e)
                })
        
        # Commit all changes
        logger.debug("Committing all database transactions...")
        conn.commit()
        logger.info("Database transactions committed successfully")
        
        # Summary
        billed = sum(1 for r in results if r['status'] == 'billed')
        stopped_negative = sum(1 for r in results if r['status'] == 'stopped_negative_balance')
        api_failed = sum(1 for r in results if r['status'] == 'balance_zero_api_failed')
        errors = sum(1 for r in results if r['status'] == 'error')

        logger.info("=" * 60)
        logger.info("BILLING SUMMARY")
        logger.info("=" * 60)
        logger.info(f"Total rentals processed: {len(rentals)}")
        logger.info(f"Billed: {billed}")
        logger.info(f"Stopped (negative balance): {stopped_negative}")
        logger.info(f"API Failed (balance=0, rental active): {api_failed}")
        logger.info(f"Errors: {errors}")
        if api_failed > 0:
            logger.warning("=" * 60)
            logger.warning(f"ATTENTION: {api_failed} rental(s) have API failures")
            logger.warning("These rentals have balance set to 0 but remain active")
            logger.warning("They will be retried on the next billing cycle")
            logger.warning("=" * 60)
        logger.info("=" * 60)
        logger.info("Hourly billing process completed")
        logger.info("=" * 60)
        
    except Exception as e:
        logger.error(f"Database error: {e}", exc_info=True)
        if conn:
            logger.debug("Rolling back database transactions due to error")
            conn.rollback()
        raise
    finally:
        if conn:
            logger.debug("Closing database connection")
            conn.close()
            logger.debug("Database connection closed")


if __name__ == '__main__':
    main()


# can be added handler for small amounts: if user has balance only for 1 hour, rents before hour starts, proxy will be stopped the next hour,
# so user wont get full hour of proxy rent. (though for rubles it's not even possible to top-up for this small amount, only for crypto)
# 1 rub = 0.11 eur (= 7 hours of proxy rent)
