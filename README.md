# Socks5.Website

### !

A modern, real-time web application for renting anonymous SOCKS5 proxy servers with automated hourly billing.

## How It Works

This project is a complete proxy rental platform that allows users to rent SOCKS5 proxies on-demand. The system consists of several integrated components:

- **Panel** (`panel/`): The main web application providing the user interface for browsing, renting, and managing proxy servers
- **Background Billing** (`bg_charger/`): A Python-based hourly billing script that runs as a cronjob to automatically charge users for active rentals
- **Database**: PostgreSQL database that tracks user accounts, server inventory, rentals, transactions, and billing history. Use DBeaver Community Edition (DBeaver-CE) to manage and insert new servers
- **Proxy Servers**: Each proxy server runs Dante (SOCKS5 server) alongside a custom `micro-service` - *controller* that the main panel communicates with via a secret key for server management and authentication
- **Payment Systems**: Two payment gateways are already integrated for processing user top-ups

The entire system is fully automated: users create anonymous accounts instantly, rent servers with immediate activation, and are billed hourly through the background cronjob. Proxy details and credentials are delivered immediately upon rental.

A modern, real-time web application for renting anonymous SOCKS5 proxy servers. Socks5.Website provides users with an intuitive interface to browse, rent, and manage multiple proxy servers with real-time network statistics and latency measurements.

## Overview

Socks5.Website is not a traditional website but rather a single-page web application (SPA) that functions as a software interface. It enables users to make informed decisions about proxy server rentals by providing comprehensive real-time metrics, allowing them to rent proxies on-demand, and manage their accounts with full anonymity.

## Key Features

### 1. Server Browsing & Discovery
- **Live Server Directory**: Browse a list of proxy servers across multiple geographic locations (US, Europe, Asia-Pacific, Australia)
- **Real-Time Ping Measurement**: JavaScript-based latency measurement to each server, continuously updated every few seconds to help you choose the fastest option
- **Advanced Filtering**: Search servers by location, name, or other criteria
- **Smart Sorting**: Sort servers by price, latency, server load, or uptime percentage
- **Server Capacity**: See how many user slots are available on each server; fully booked servers are disabled

### 2. Server Statistics
Each server displays comprehensive metrics to inform your decision:
- **Ping (Latency)**: Real-time latency measurement in milliseconds; critical for determining responsiveness
- **Server Load**: Current CPU/network load percentage; lower is better for performance
- **Bandwidth Capacity**: Available network bandwidth (e.g., "950 Mbps")
- **Uptime**: Historical uptime percentage (e.g., 99.98%)
- **Active Since**: Date the server was added to the network
- **User Slots**: Number of active users and total capacity
- **Network Location**: Geographic location with flag emoji for quick visual reference

### 3. Rental System
- **Hourly Pricing**: Each server has a fixed price per rental hour (billed in EUR)
- **Multi-Server Rentals**: Rent one or multiple servers simultaneously
- **Immediate Activation**: Proxies are available immediately upon purchase
- **Flexible Duration**: Change, upgrade, or downgrade servers at any time
- **Auto-Renewal**: Active rentals automatically renew hourly, with balance deducted automatically
- **Release Anytime**: Cancel a rental immediately to stop charges (refunds for partial hours depend on subscription tier)

### 4. Account & Balance Management
- **Anonymous Accounts**: No registration required; create an account instantly with no personal information
- **Cryptocurrency Top-Ups**: Fund your account using various cryptocurrencies (Bitcoin, Ethereum, Litecoin, Monero, USDT)
- **EUR Balance**: Single account balance in Euros; all transactions displayed in EUR
- **Secret Key System**: Every account has a unique secret key that can be shared to access your account from other devices/browsers
- **Account Portability**: Paste your secret key on another device to instantly access your rentals and balance

### 5. Real-Time Dashboard
The sidebar displays live network and account information:
- **Account Status**: Anonymous mode indicator
- **Active Rentals**: Count of currently running proxy rentals
- **Total Spent**: Cumulative spending across all rentals in EUR
- **Secret Key**: Unique identifier to access your account from anywhere
- **Network Statistics**: Average ping across all servers, online server count, average load
- **My Rentals**: Active rentals with expiration time and hourly rate

### 6. Security & Privacy
- **No Registration**: No email, password, or personal data required
- **Anonymous**: Complete anonymity; no account linking to personal identity
- **Secret Key Based**: Access your account using a cryptographic key instead of credentials
- **Client-Side Processing**: Most operations happen locally in your browser
- **Cryptocurrency Payments**: Privacy-preserving payment method that doesn't require personal information

## How to Use

### Getting Started
1. Open socks5.website in your browser (any device, any location)
2. Your anonymous account is created automatically with a unique secret key
3. Your initial balance is displayed in the header (top-up required to rent servers)

### Finding the Right Server
1. Navigate to the **Browse Servers** tab
2. Review the list of available servers with their current statistics
3. Pay attention to **Ping** (lower is faster) and **Load** (lower is better)
4. Use the **Search** bar to filter by location or name
5. Use the **Sort** dropdown to organize servers by price, ping, load, or uptime
6. Click **Details** on any server to see full specifications before renting

### Renting a Server
1. Click the **Rent** button on any available server (disabled if fully booked)
2. The server is instantly activated and added to your **My Rentals** list
3. Your account balance is immediately debited for the first hour
4. Hourly charges continue automatically until you release the server
5. You can rent multiple servers at the same time

### Managing Active Rentals
1. Go to the **My Rentals** tab to see all currently rented servers
2. Each rental shows the remaining time until the next hourly charge
3. Click **Release** to immediately stop the rental and prevent further charges
4. Monitor your balance in the header to ensure sufficient funds

### Accessing Your Account from Another Device
1. Click the **Account** button (⚙️) in the header
2. Copy your **Secret Key**
3. Open socks5.website on another device/browser
4. Click **Account** and paste your secret key
5. Your account, balance, and active rentals are instantly available

### Topping Up Your Balance
1. Click the **Top Up** (💳) button in the header
2. Enter the amount you wish to add (minimum €5)
3. Select your preferred cryptocurrency (BTC, ETH, LTC, XMR, USDT)
4. Follow the payment instructions to send the exact amount
5. Your balance is credited automatically upon confirmation

## Technical Architecture

### Frontend
- **Single-Page Application (SPA)**: Vanilla JavaScript (no frameworks)
- **Responsive Design**: Mobile-first approach; works on phones, tablets, and desktops
- **Real-Time Updates**: Live server statistics with periodic refresh every 3 seconds
- **Real-Time Ping Measurement**: JavaScript measures latency to each server
- **Local State Management**: App state stored in JavaScript; syncs with backend on changes

### Backend (Conceptual)
- **API Endpoints**: RESTful API for fetching servers, managing rentals, and account operations
- **Authentication**: Secret key-based authentication (no passwords)
- **Billing System**: Automated hourly billing via cron jobs or background tasks
- **Cryptocurrency Integration**: Payment gateway integration for Bitcoin, Ethereum, etc.
- **Database**: User accounts, rental history, server information, transaction logs

### Data Flow
1. **Initial Load**: Fetch server list and account info from backend
2. **Real-Time Updates**: Periodic fetch of server statistics and account balance
3. **Rental**: POST request to create rental; backend deducts balance and activates proxy
4. **Hourly Billing**: Background job runs hourly to charge active rentals
5. **Account Access**: Secret key validates identity without passwords

## Pricing Model

- **Server Costs**: Vary by location and capacity (typically €1.50 - €4.50 per hour)
- **Billing Cycle**: Hourly; charges deducted automatically from balance
- **Minimum Top-Up**: €5.00
- **Maximum Top-Up**: €10,000.00
- **Exchange Rate**: Crypto to EUR rate determined at payment time

## Supported Cryptocurrencies

- **Bitcoin (BTC)**: High security, widely accepted
- **Ethereum (ETH)**: Fast confirmation times
- **Litecoin (LTC)**: Quick and cheap transactions
- **Monero (XMR)**: Enhanced privacy and anonymity
- **Tether (USDT)**: Stablecoin for predictable pricing

## Account Security Considerations

- **Secret Key**: Treat like a password; never share with untrusted parties
- **No Recovery**: If you lose your secret key, the account cannot be recovered
- **Key Rotation**: Generate a new key anytime from Account settings (invalidates old key)
- **No Backup**: No email recovery or password reset; keys are final

## Mobile Experience

Socks5.Website is fully optimized for mobile devices:
- **Touch-Friendly Buttons**: Large tap targets for easy mobile use
- **Responsive Layout**: Single-column layout on small screens
- **Optimized Performance**: Lightweight, fast-loading interface
- **Mobile Browsers**: Works in Chrome, Safari, Firefox, Edge on iOS and Android

## API Overview (Backend Specification)

### Server Endpoints
- `GET /api/servers` - Fetch all available servers with current statistics
- `GET /api/servers/:id` - Get detailed information for a specific server

### Rental Endpoints
- `POST /api/rentals` - Create a new rental (requires balance check)
- `GET /api/rentals` - Fetch user's active rentals
- `DELETE /api/rentals/:id` - Release/cancel a rental

### Account Endpoints
- `GET /api/account` - Fetch account info and balance (authenticated via secret key)
- `POST /api/account/topup` - Initiate a cryptocurrency top-up
- `GET /api/account/topup/:txid` - Check status of a pending top-up
- `POST /api/account/key/generate` - Generate a new secret key

### Billing
- `POST /api/billing/charge` - (Internal) Charge active rentals hourly
- `GET /api/billing/history` - Fetch transaction and rental history

## FAQ

**Q: Is my account truly anonymous?**
A: Yes. Socks5.Website requires no personal information. Accounts are identified only by secret keys.

**Q: What happens if my balance runs out?**
A: All active rentals are immediately paused. You must top up your balance to reactivate them.

**Q: Can I get a refund for partial hours?**
A: Refund policies depend on your account tier. Standard accounts are charged hourly with no prorated refunds.

**Q: How accurate is the ping measurement?**
A: Ping values are measured in real-time by your browser and update every few seconds. Actual network conditions may vary.

**Q: Can I share my secret key with others?**
A: Yes, but this means they can access your account and spend your balance. Share only with trusted individuals.

**Q: What if I forget my secret key?**
A: There is no recovery mechanism. A new key must be generated, which invalidates the old one. Save your key securely.

**Q: Are the proxies tracked or logged?**
A: Our privacy policy prohibits extensive logging. However, we record minimal metadata for billing and fraud prevention.

**Q: How many servers can I rent simultaneously?**
A: Unlimited; rent as many as you need (limited only by available balance and server capacity).

**Q: What if a server goes offline?**
A: Your rental is paused; charges continue. You may release the rental or wait for the server to return online.

---

**Last Updated**: March 2026  
**Version**: 1.0.0  
**License**: Proprietary




dev notes:

add conversion rate to platega payment option: e.g. hardcoded 97.5 rub per 1 eur
