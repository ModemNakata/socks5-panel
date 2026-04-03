// Get provider from URL query parameter
const urlParams = new URLSearchParams(window.location.search);
// const provider = urlParams.get('provider') || 'monero';
const provider = urlParams.get('provider');
const method = urlParams.get('method');
const amount = urlParams.get('amount');

const statusEl = document.getElementById('status');

// Request payment URL from backend
async function getPaymentUrl() {
    try {
        let url = `/api/payment/process/${provider}`;
        const queryParams = new URLSearchParams();
        
        if (method) {
            queryParams.set('method', method);
        }
        if (amount) {
            queryParams.set('amount', amount);
        }
        
        const queryString = queryParams.toString();
        if (queryString) {
            url += '?' + queryString;
        }
        
        const response = await fetch(url, { // add query string param: ?&method= here (to choose coin in cryptowrap)
            method: 'POST',
        });
        const data = await response.json();

        if (data.success && data.payment_url) {
            // Payment URL ready, redirect
            statusEl.textContent = 'Redirecting to payment gateway...';
            window.location.href = data.payment_url;
        } else if (data.error) {
            statusEl.textContent = `Error: ${data.error}`;
        } else {
            statusEl.textContent = 'Preparing payment details...';
            setTimeout(getPaymentUrl, 1000);
        }
    } catch (error) {
        console.error('Payment request failed:', error);
        statusEl.textContent = 'Connection error, retrying...';
        setTimeout(getPaymentUrl, 2000);
    }
}

// Start
getPaymentUrl();
