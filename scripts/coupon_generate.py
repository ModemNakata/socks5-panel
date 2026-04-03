#!/usr/bin/env python3
"""
Coupon management CLI script.

This script allows you to create, delete, and list coupons for the panel system.
Coupons can be used to give users free credit (balance) with limited usage.

Usage:
    python coupon_generate.py create --balance 5.00 --max-uses 100 [--code PROMO123] [--expires 2026-12-31]
    python coupon_generate.py delete --code PROMO123
    python coupon_generate.py list [--active-only]
    python coupon_generate.py info --code PROMO123
"""

import os
import sys
import argparse
import uuid
import re
from datetime import datetime, timezone
from decimal import Decimal, ROUND_HALF_UP
from dotenv import load_dotenv
import psycopg2
from psycopg2.extras import RealDictCursor
import secrets
import string

# Configure logging
import logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Load environment variables
env_path = os.path.join(os.path.dirname(__file__), '..', '.env')
logger.debug(f"Loading environment from: {env_path}")
load_dotenv(env_path)

DB_URL = os.getenv('DB_URL')

if not DB_URL:
    raise ValueError("DB_URL not found in .env file")


def generate_coupon_code(prefix: str = None, length: int = 12) -> str:
    """
    Generate a random coupon code.
    
    Format: [PREFIX-]XXXXXX (uppercase alphanumeric)
    
    Args:
        prefix: Optional prefix (e.g., "PROMO", "WELCOME")
        length: Length of the random part (default: 12)
    
    Returns:
        Generated coupon code (e.g., "PROMO-X7K9M2P4Q8R3")
    """
    alphabet = string.ascii_uppercase + string.digits
    random_part = ''.join(secrets.choice(alphabet) for _ in range(length))
    
    if prefix:
        return f"{prefix.upper()}-{random_part}"
    return random_part


def create_coupon(conn, code: str, balance: Decimal, max_uses: int, 
                  expires_at: datetime = None, created_by: str = "cli", 
                  is_active: bool = True) -> dict:
    """
    Create a new coupon in the database.
    
    Args:
        conn: Database connection
        code: Coupon code (unique identifier)
        balance: Amount of credit this coupon gives (in EUR)
        max_uses: Maximum number of times this coupon can be redeemed
        expires_at: Optional expiration datetime
        created_by: Identifier of who created this coupon
        is_active: Whether the coupon is active
    
    Returns:
        dict with coupon information
    """
    logger.info(f"Creating coupon: code={code}, balance={balance}, max_uses={max_uses}")
    
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Check if code already exists
        cur.execute("SELECT code FROM coupons WHERE code = %s", (code,))
        if cur.fetchone():
            raise ValueError(f"Coupon code '{code}' already exists")
        
        # Insert new coupon
        query = """
            INSERT INTO coupons (id, code, balance_amount, max_uses, used_count, 
                                is_active, expires_at, created_at, created_by)
            VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            RETURNING id, code, balance_amount, max_uses, used_count, 
                     is_active, expires_at, created_at, created_by
        """
        
        coupon_id = str(uuid.uuid4())
        now = datetime.now(timezone.utc)

        # Convert expires_at to string if it's a datetime
        expires_at_str = expires_at.isoformat() if expires_at else None

        cur.execute(query, (
            coupon_id,
            code,
            str(balance),
            max_uses,
            0,  # used_count starts at 0
            is_active,
            expires_at_str,
            now.isoformat(),
            created_by
        ))
        
        result = cur.fetchone()
        logger.info(f"Coupon created successfully with ID: {coupon_id}")
        
        return dict(result)


def delete_coupon(conn, code: str) -> dict:
    """
    Delete a coupon from the database.
    
    Args:
        conn: Database connection
        code: Coupon code to delete
    
    Returns:
        dict with deletion result
    """
    logger.info(f"Deleting coupon: code={code}")
    
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Get coupon info before deletion
        cur.execute("""
            SELECT id, code, balance_amount, max_uses, used_count, is_active
            FROM coupons
            WHERE code = %s
        """, (code,))
        
        coupon = cur.fetchone()
        if not coupon:
            raise ValueError(f"Coupon code '{code}' not found")
        
        # Delete the coupon (cascade will handle redemptions)
        cur.execute("DELETE FROM coupons WHERE code = %s", (code,))
        
        logger.info(f"Coupon deleted successfully: {code}")
        
        return {
            'success': True,
            'deleted_coupon': dict(coupon)
        }


def deactivate_coupon(conn, code: str) -> dict:
    """
    Deactivate a coupon (soft delete - keeps history but prevents further redemptions).
    
    Args:
        conn: Database connection
        code: Coupon code to deactivate
    
    Returns:
        dict with deactivation result
    """
    logger.info(f"Deactivating coupon: code={code}")
    
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Check if coupon exists
        cur.execute("SELECT id, code, is_active FROM coupons WHERE code = %s", (code,))
        coupon = cur.fetchone()
        
        if not coupon:
            raise ValueError(f"Coupon code '{code}' not found")
        
        if not coupon['is_active']:
            logger.info(f"Coupon '{code}' is already deactivated")
            return {
                'success': True,
                'message': f"Coupon '{code}' was already deactivated",
                'coupon': dict(coupon)
            }
        
        # Deactivate the coupon
        cur.execute("""
            UPDATE coupons
            SET is_active = false
            WHERE code = %s
            RETURNING id, code, is_active
        """, (code,))
        
        result = cur.fetchone()
        logger.info(f"Coupon deactivated successfully: {code}")
        
        return {
            'success': True,
            'message': f"Coupon '{code}' has been deactivated",
            'coupon': dict(result)
        }


def list_coupons(conn, active_only: bool = False, limit: int = 50) -> list:
    """
    List coupons from the database.
    
    Args:
        conn: Database connection
        active_only: If True, only show active coupons
        limit: Maximum number of results to return
    
    Returns:
        list of coupon dictionaries
    """
    logger.info(f"Listing coupons (active_only={active_only}, limit={limit})")
    
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        query = """
            SELECT id, code, balance_amount, max_uses, used_count, 
                   is_active, expires_at, created_at, created_by
            FROM coupons
        """
        
        params = []
        conditions = []
        
        if active_only:
            conditions.append("is_active = true")
        
        if conditions:
            query += " WHERE " + " AND ".join(conditions)
        
        query += " ORDER BY created_at DESC LIMIT %s"
        params.append(limit)
        
        cur.execute(query, params)
        results = cur.fetchall()
        
        logger.info(f"Found {len(results)} coupons")
        
        return [dict(row) for row in results]


def get_coupon_info(conn, code: str) -> dict:
    """
    Get detailed information about a specific coupon.
    
    Args:
        conn: Database connection
        code: Coupon code to look up
    
    Returns:
        dict with coupon information and redemption stats
    """
    logger.info(f"Getting info for coupon: code={code}")
    
    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Get coupon info
        cur.execute("""
            SELECT id, code, balance_amount, max_uses, used_count, 
                   is_active, expires_at, created_at, created_by
            FROM coupons
            WHERE code = %s
        """, (code,))
        
        coupon = cur.fetchone()
        if not coupon:
            raise ValueError(f"Coupon code '{code}' not found")
        
        # Get recent redemptions
        cur.execute("""
            SELECT cr.redeemed_at, cr.amount_added, us.id as user_id
            FROM coupon_redemptions cr
            JOIN user_session us ON cr.user_id = us.id
            WHERE cr.coupon_id = %s
            ORDER BY cr.redeemed_at DESC
            LIMIT 10
        """, (coupon['id'],))
        
        redemptions = cur.fetchall()
        
        result = dict(coupon)
        result['recent_redemptions'] = [dict(r) for r in redemptions]
        
        # Calculate usage percentage
        result['usage_percentage'] = (coupon['used_count'] / coupon['max_uses'] * 100) if coupon['max_uses'] > 0 else 0
        
        return result


def format_coupon(coupon: dict) -> str:
    """Format a coupon dictionary for display."""
    status = "✓ ACTIVE" if coupon['is_active'] else "✗ INACTIVE"
    expires = coupon['expires_at'].strftime('%Y-%m-%d %H:%M') if coupon['expires_at'] else "Never"
    usage = f"{coupon['used_count']}/{coupon['max_uses']} ({coupon.get('usage_percentage', 0):.1f}%)"
    
    lines = [
        f"Code: {coupon['code']}",
        f"  Status: {status}",
        f"  Balance: €{coupon['balance_amount']}",
        f"  Usage: {usage}",
        f"  Expires: {expires}",
        f"  Created: {coupon['created_at'].strftime('%Y-%m-%d %H:%M')} by {coupon['created_by']}",
    ]
    
    if 'recent_redemptions' in coupon and coupon['recent_redemptions']:
        lines.append("  Recent redemptions:")
        for r in coupon['recent_redemptions'][:5]:
            lines.append(f"    - {r['redeemed_at'].strftime('%Y-%m-%d %H:%M')} | User: {r['user_id']} | €{r['amount_added']}")
    
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Coupon management CLI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Create a coupon with auto-generated code
  %(prog)s create --balance 5.00 --max-uses 100
  
  # Create a coupon with custom code
  %(prog)s create --code WELCOME2026 --balance 10.00 --max-uses 50
  
  # Create a coupon with expiration date
  %(prog)s create --balance 5.00 --max-uses 100 --expires 2026-12-31
  
  # Create a coupon with prefix
  %(prog)s create --balance 3.00 --max-uses 200 --prefix PROMO
  
  # List all coupons
  %(prog)s list
  
  # List only active coupons
  %(prog)s list --active-only
  
  # Get info about a specific coupon
  %(prog)s info --code WELCOME2026
  
  # Deactivate a coupon (soft delete)
  %(prog)s deactivate --code WELCOME2026
  
  # Delete a coupon permanently
  %(prog)s delete --code WELCOME2026
        """
    )
    
    subparsers = parser.add_subparsers(dest='command', help='Command to execute')
    
    # Create command
    create_parser = subparsers.add_parser('create', help='Create a new coupon')
    create_parser.add_argument('--balance', type=float, required=True, 
                               help='Credit amount in EUR (e.g., 5.00)')
    create_parser.add_argument('--max-uses', type=int, required=True,
                               help='Maximum number of redemptions (e.g., 100)')
    create_parser.add_argument('--code', type=str, default=None,
                               help='Custom coupon code (auto-generated if not provided)')
    create_parser.add_argument('--prefix', type=str, default=None,
                               help='Code prefix for auto-generated codes (e.g., PROMO)')
    create_parser.add_argument('--expires', type=str, default=None,
                               help='Expiration date (YYYY-MM-DD format)')
    create_parser.add_argument('--created-by', type=str, default='cli',
                               help='Creator identifier (default: cli)')
    
    # Delete command
    delete_parser = subparsers.add_parser('delete', help='Delete a coupon permanently')
    delete_parser.add_argument('--code', type=str, required=True,
                               help='Coupon code to delete')
    delete_parser.add_argument('--force', action='store_true',
                               help='Skip confirmation prompt')
    
    # Deactivate command
    deactivate_parser = subparsers.add_parser('deactivate', help='Deactivate a coupon (soft delete)')
    deactivate_parser.add_argument('--code', type=str, required=True,
                                   help='Coupon code to deactivate')
    
    # List command
    list_parser = subparsers.add_parser('list', help='List coupons')
    list_parser.add_argument('--active-only', action='store_true',
                             help='Show only active coupons')
    list_parser.add_argument('--limit', type=int, default=50,
                             help='Maximum number of results (default: 50)')
    
    # Info command
    info_parser = subparsers.add_parser('info', help='Get detailed info about a coupon')
    info_parser.add_argument('--code', type=str, required=True,
                             help='Coupon code to look up')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        sys.exit(1)
    
    conn = None
    try:
        logger.debug("Connecting to database...")
        conn = psycopg2.connect(DB_URL)
        logger.info("Database connection established")
        
        if args.command == 'create':
            # Validate balance
            if args.balance <= 0:
                raise ValueError("Balance must be positive")
            
            # Validate max_uses
            if args.max_uses <= 0:
                raise ValueError("max_uses must be positive")
            
            # Parse expiration date
            expires_at = None
            if args.expires:
                try:
                    expires_at = datetime.strptime(args.expires, '%Y-%m-%d')
                    expires_at = expires_at.replace(tzinfo=timezone.utc)
                except ValueError:
                    raise ValueError(f"Invalid date format: {args.expires}. Use YYYY-MM-DD")
            
            # Generate or use provided code
            if args.code:
                code = args.code.upper()
            else:
                code = generate_coupon_code(prefix=args.prefix)
            
            # Create coupon
            balance = Decimal(str(args.balance)).quantize(Decimal('0.01'), rounding=ROUND_HALF_UP)
            coupon = create_coupon(
                conn, 
                code=code, 
                balance=balance, 
                max_uses=args.max_uses,
                expires_at=expires_at,
                created_by=args.created_by
            )
            
            conn.commit()
            
            print("\n✓ Coupon created successfully!\n")
            print(f"  Code: {coupon['code']}")
            print(f"  Balance: €{coupon['balance_amount']}")
            print(f"  Max Uses: {coupon['max_uses']}")
            if expires_at:
                print(f"  Expires: {args.expires}")
            print(f"\n  Share this code with users to let them claim €{coupon['balance_amount']} credit!")
            print("  Users can redeem it in the 'Top Up Balance' modal.\n")
        
        elif args.command == 'delete':
            if not args.force:
                confirm = input(f"Are you sure you want to permanently delete coupon '{args.code}'? [y/N]: ")
                if confirm.lower() != 'y':
                    print("Cancelled.")
                    sys.exit(0)
            
            result = delete_coupon(conn, args.code)
            conn.commit()
            
            print("\n✓ Coupon deleted successfully!\n")
            print(f"  Deleted code: {result['deleted_coupon']['code']}")
            print(f"  Balance was: €{result['deleted_coupon']['balance_amount']}")
            print(f"  Uses: {result['deleted_coupon']['used_count']}/{result['deleted_coupon']['max_uses']}\n")
        
        elif args.command == 'deactivate':
            result = deactivate_coupon(conn, args.code)
            conn.commit()
            
            print(f"\n✓ {result['message']}\n")
        
        elif args.command == 'list':
            coupons = list_coupons(conn, active_only=args.active_only, limit=args.limit)
            
            if not coupons:
                print("\nNo coupons found.\n")
            else:
                print(f"\n{'='*70}")
                print(f"Found {len(coupons)} coupon(s):\n")
                
                for coupon in coupons:
                    # Calculate usage percentage for display
                    usage_pct = (coupon['used_count'] / coupon['max_uses'] * 100) if coupon['max_uses'] > 0 else 0
                    coupon['usage_percentage'] = usage_pct
                    print(format_coupon(coupon))
                    print(f"{'-'*70}")
                
                print()
        
        elif args.command == 'info':
            coupon = get_coupon_info(conn, args.code)
            coupon['usage_percentage'] = (coupon['used_count'] / coupon['max_uses'] * 100) if coupon['max_uses'] > 0 else 0
            
            print(f"\n{'='*70}")
            print(format_coupon(coupon))
            print(f"{'='*70}\n")
    
    except ValueError as e:
        logger.error(f"Error: {e}")
        sys.exit(1)
    except psycopg2.Error as e:
        logger.error(f"Database error: {e}")
        if conn:
            conn.rollback()
        sys.exit(1)
    except Exception as e:
        logger.error(f"Unexpected error: {e}", exc_info=True)
        if conn:
            conn.rollback()
        sys.exit(1)
    finally:
        if conn:
            conn.close()
            logger.debug("Database connection closed")


if __name__ == '__main__':
    main()
