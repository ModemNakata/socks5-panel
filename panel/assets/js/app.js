const PING_MODE = 'median'; // 'best' | 'average' | 'median'

let servers = [];

let sortColumn = 'price';
let sortDirection = 'asc';
let rentals = [];
let pingMeasured = false;
let currentRentingServerId = null;

async function fetchServers() {
    try {
        const response = await fetch('/api/servers');
        const data = await response.json();
        console.log('API response:', data);
        servers = data.map(s => ({
            ...s,
            countryCode: getCountryCode(s.country),
            price: parseFloat(s.price),
            ping: null,
            pingMeasured: false,
            availableSlots: parseInt(s.slots_available)
        }));
        console.log('Servers loaded:', servers.length);
        await fetchRentals();
        renderTable();
    } catch (error) {
        console.error('Failed to fetch servers:', error);
    }
}

async function fetchRentals() {
    try {
        const response = await fetch('/api/rentals');
        const data = await response.json();

        if (data.success && data.rentals) {
            rentals = data.rentals;
            updateDashboard();
        }
    } catch (error) {
        console.error('Failed to fetch rentals:', error);
    }
}

function updateServerSlots(serverId, delta) {
    const server = servers.find(s => s.id === serverId);
    if (server) {
        server.availableSlots = Math.max(0, server.availableSlots + delta);
    }
}

function getCountryCode(country) {
    const codes = {
        'United States': 'US', 'Germany': 'DE', 'United Kingdom': 'GB',
        'Netherlands': 'NL', 'France': 'FR', 'Canada': 'CA',
        'Japan': 'JP', 'Australia': 'AU', 'Singapore': 'SG', 'Switzerland': 'CH',
        'Finland': 'FI',
    };
    return codes[country] || 'XX';
}

function getFlag(countryCode) {
    const flags = {
        US: '🇺🇸', DE: '🇩🇪', GB: '🇬🇧', NL: '🇳🇱', FR: '🇫🇷',
        CA: '🇨🇦', JP: '🇯🇵', AU: '🇦🇺', SG: '🇸🇬', CH: '🇨🇭',
        FI: '🇫🇮',
    };
    return flags[countryCode] || '🌐';
}

function formatPrice(price) {
    return `€${price.toFixed(2)}`;
}

function sortServers(servers) {
    return [...servers].sort((a, b) => {
        let aVal = a[sortColumn];
        let bVal = b[sortColumn];

        if (sortColumn === 'country') {
            aVal = a.country.toLowerCase();
            bVal = b.country.toLowerCase();
        }

        if (sortDirection === 'asc') {
            return aVal > bVal ? 1 : -1;
        }
        return aVal < bVal ? 1 : -1;
    });
}

function renderTable() {
    const tbody = document.getElementById('servers-tbody');

    if (servers.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="7" style="text-align: center; padding: 3rem;">
                    <p style="color: var(--text-muted); font-size: 1.1rem;">No servers available at the moment. Please check back soon.</p>
                </td>
            </tr>
        `;
        renderRentedCredentials();
        updateTabCounters();
        return;
    }

    const sortedServers = sortServers(servers);

    tbody.innerHTML = sortedServers.map(server => {
        const isFull = server.availableSlots === 0;
        const isOnline = server.pingMeasured && server.ping !== null;
        const rentButtonText = isFull ? 'FULL' : 'Rent';
        const rentButtonDisabled = isFull;

        return `
        <tr data-id="${server.id}">
            <td>
                <div class="status-cell">
                    <span class="online-dot ${isOnline ? '' : 'offline'}"></span>
                    <span data-status="${server.id}">${isOnline ? 'Online' : 'Offline'}</span>
                </div>
            </td>
            <td>
                <div class="country-cell">
                    <span class="country-flag">${getFlag(server.countryCode)}</span>
                    <div class="country-details">
                        <span class="country-name">${server.country}</span>
                        <span class="country-info">${server.codename}</span>
                    </div>
                </div>
            </td>
            <td>
                <div class="ping-cell">
                    <button class="ping-btn" data-ping-btn="${server.id}" onclick="measurePingSingle('${server.id}')" ${server.ping !== null ? '' : 'disabled'}>
                        ${server.ping !== null ? `${server.ping} <span class="ping-unit">ms</span>` : '<span class="ping-loading"></span>'}
                    </button>
                </div>
            </td>
            <td>${formatPrice(server.price)}<span class="unit">/mo</span></td>
            <td>${server.speed}<span class="unit"> Mbps</span></td>
            <td>${server.availableSlots}<span class="unit"> available</span></td>
            <td>
                <button class="btn-rent rent" onclick="openRentModal('${server.id}')" ${rentButtonDisabled ? 'disabled' : ''}>${rentButtonText}</button>
            </td>
        </tr>
    `;
    }).join('');

    renderRentedCredentials();
    updateTabCounters();

    // Update scroll indicators
    if (typeof updateTableScrollIndicators === 'function') {
        setTimeout(updateTableScrollIndicators, 50);
    }

    if (!pingMeasured) {
        pingMeasured = true;
        console.log(`Starting ping measurement for ${servers.length} servers...`);
        measurePingsSequential(0);
    }
}

function measurePingsSequential(index) {
    if (index >= servers.length) {
        console.log('All ping measurements completed');
        return;
    }

    const server = servers[index];
    console.log(`Measuring ping ${index + 1}/${servers.length}: ${server.codename}`);

    measurePing(server.id, () => {
        setTimeout(() => measurePingsSequential(index + 1), 100);
    });
}

function renderRentedCredentials() {
    const container = document.getElementById('rented-credentials');

    if (rentals.length === 0) {
        container.innerHTML = '<p style="color: var(--text-muted-dark); padding: 0 2rem;">No proxies rented yet.</p>';
        return;
    }

    container.innerHTML = rentals.map((rental, index) => `
        <div class="rented-credential-row">
            <span class="rented-country">${getFlag(rental.country_code || getCountryCode(rental.country))} ${rental.server_codename}</span>
            <code class="proxy-creds" onclick="copyCredentials('${rental.username}', '${rental.password}', '${rental.server_codename}.socks5.website', '${rental.port}')">socks5://${rental.username}:${rental.password}@${rental.server_codename}.socks5.website:${rental.port}</code>
            <button class="btn-stop-rental" onclick="stopRental('${rental.id}')">Stop</button>
        </div>
    `).join('');

    updateTabCounters();
}

async function measurePing(serverId, callback) {
    const server = servers.find(s => s.id === serverId);
    if (!server) {
        if (callback) callback();
        return;
    }

    const pingEl = document.querySelector(`[data-ping="${serverId}"]`);
    if (pingEl) {
        pingEl.innerHTML = '<span class="ping-loading"></span>';
    }

    try {
        const samples = 3;
        const pings = [];

        for (let i = 0; i < samples; i++) {
            const start = performance.now();
            await fetch(`https://${server.codename}.socks5.website/ping?t=${performance.now()}-${i}`, {
                method: 'HEAD',
                mode: 'no-cors',
                cache: 'no-store',
            });
            const elapsed = performance.now() - start;
            pings.push(elapsed);
        }

        // Sort ascending, take the best 3 to filter out outlier spikes
        pings.sort((a, b) => a - b);
        const bestThree = pings.slice(0, 3);
        let rawPing;
        if (PING_MODE === 'best') {
            rawPing = bestThree[0];
        } else if (PING_MODE === 'median') {
            rawPing = bestThree[1];
        } else {
            rawPing = bestThree.reduce((a, b) => a + b, 0) / bestThree.length;
        }
        // Enforce minimum of 1ms
        const ping = Math.max(1, Math.floor(rawPing));
        console.log(`${server.codename}: all=[${pings.map(p => p.toFixed(1)).join(', ')}] best3=[${bestThree.map(p => p.toFixed(1)).join(', ')}] result=${ping}ms`);

        server.ping = ping;
        updatePing(serverId, server.ping);
    } catch (err) {
        console.error(`Ping failed for ${server.codename}:`, err);
    }

    if (callback) callback();
}

function measurePingSingle(serverId) {
    const pingBtn = document.querySelector(`[data-ping-btn="${serverId}"]`);
    if (pingBtn) {
        pingBtn.classList.add('ping-measuring');
        pingBtn.disabled = true;
    }

    measurePing(serverId, () => {
        if (pingBtn) {
            pingBtn.classList.remove('ping-measuring');
            pingBtn.disabled = false;
            updatePingDisplay(serverId);
        }
    });
}

function updatePingDisplay(serverId) {
    const server = servers.find(s => s.id === serverId);
    if (!server) return;

    const pingBtn = document.querySelector(`[data-ping-btn="${serverId}"]`);
    if (pingBtn) {
        pingBtn.innerHTML = `${server.ping} <span class="ping-unit">ms</span>`;
    }
}

function updatePing(serverId, ping) {
    const server = servers.find(s => s.id === serverId);
    const pingBtn = document.querySelector(`[data-ping-btn="${serverId}"]`);
    if (pingBtn) {
        pingBtn.innerHTML = `${ping} <span class="ping-unit">ms</span>`;
        pingBtn.disabled = false;
    }

    const statusEl = document.querySelector(`[data-status="${serverId}"]`);
    if (statusEl) {
        statusEl.textContent = 'Online';
    }

    const dotEl = document.querySelector(`tr[data-id="${serverId}"] .online-dot`);
    if (dotEl) {
        dotEl.classList.remove('offline');
    }

    if (server) {
        server.ping = ping;
        server.pingMeasured = true;
    }
}

function showCredentialsHelp(event) {
    event.stopPropagation();
    document.getElementById('help-modal').classList.add('active');
}

function openRentModal(serverId) {
    const server = servers.find(s => s.id === serverId);
    if (!server) return;

    currentRentingServerId = serverId;
    const maxSlots = server.availableSlots;

    document.getElementById('rent-modal-server').textContent = `Server: ${server.codename} (${server.country})`;

    const sliderContainer = document.querySelector('.slider-container');
    const slider = document.getElementById('slots-slider');

    if (maxSlots === 1) {
        sliderContainer.style.display = 'none';
        slider.value = 1;
        document.getElementById('slots-value').textContent = '1';
    } else {
        sliderContainer.style.display = 'block';
        slider.max = maxSlots;
        slider.value = 1;
        document.getElementById('slots-value').textContent = '1';
    }

    updateRentModalPrice(serverId, 1);
    document.getElementById('rent-modal').classList.add('active');
}

function updateRentModalPrice(serverId, slots) {
    const server = servers.find(s => s.id === serverId);
    if (!server) return;

    const totalPrice = server.price * slots;
    document.getElementById('rent-modal-price').textContent = `Total: ${formatPrice(totalPrice)}/mo`;
    document.getElementById('confirm-rent-btn').textContent = slots === 1 ? 'Rent 1 Proxy' : `Rent ${slots} Proxies`;
}

async function confirmRent() {
    const slots = parseInt(document.getElementById('slots-slider').value);
    const serverId = currentRentingServerId;

    if (!serverId) return;

    const btn = document.getElementById('confirm-rent-btn');
    btn.disabled = true;
    btn.innerHTML = '<span class="btn-spinner"></span>';

    let rentedCount = 0;
    const newRentals = [];

    try {
        for (let i = 0; i < slots; i++) {
            const response = await fetch(`/api/rent/${serverId}`, { method: 'POST' });
            const data = await response.json();

            if (data.success) {
                const rental = {
                    id: data.rental_id,
                    server_id: serverId,
                    server_codename: servers.find(s => s.id === serverId)?.codename || '',
                    username: data.username,
                    password: data.password,
                    port: data.port,
                    country: data.country
                };
                rentals.push(rental);
                newRentals.push(rental);
                rentedCount++;
            } else {
                alert(data.message || data.error || 'Failed to rent proxy');
                break;
            }
        }

        if (rentedCount > 0) {
            updateServerSlots(serverId, -rentedCount);
            renderTable();
            updateDashboard();
            document.getElementById('rent-modal').classList.remove('active');
            showCredentialsModal(newRentals);
        }
    } catch (error) {
        console.error('Rental error:', error);
        alert('Failed to communicate with server');
    } finally {
        btn.disabled = false;
        btn.innerHTML = `Rent 1 Proxy`;
    }
}

async function stopRental(rentalId) {
    const rental = rentals.find(r => r.id === rentalId);
    if (!rental) return;

    if (!confirm(`Stop renting proxy from ${rental.server_codename}?`)) return;

    // Find and disable the stop button, show spinner
    const stopBtn = document.querySelector(`[onclick="stopRental('${rentalId}')"]`);
    if (stopBtn) {
        stopBtn.disabled = true;
        stopBtn.innerHTML = '<span class="btn-spinner"></span>';
    }

    try {
        const response = await fetch(`/api/rent/${rentalId}`, { method: 'DELETE' });
        const data = await response.json();

        if (data.success) {
            const serverId = rental.server_id;
            const index = rentals.findIndex(r => r.id === rentalId);
            if (index !== -1) {
                rentals.splice(index, 1);
            }
            updateServerSlots(serverId, 1);
            renderTable();
            updateDashboard();
        } else {
            alert(data.error || 'Failed to stop rental');
            // Restore button on error
            if (stopBtn) {
                stopBtn.disabled = false;
                stopBtn.textContent = 'Stop';
            }
        }
    } catch (error) {
        console.error('Error stopping rental:', error);
        alert('Failed to communicate with server');
        // Restore button on error
        if (stopBtn) {
            stopBtn.disabled = false;
            stopBtn.textContent = 'Stop';
        }
    }
}

function updateDashboard() {
    const rentedCount = rentals.length;
    const totalPrice = rentals.reduce((sum, rental) => {
        // Use price from rental data if available, otherwise fallback to servers array
        if (rental.price) {
            return sum + parseFloat(rental.price);
        }
        const server = servers.find(s => s.id === rental.server_id);
        return sum + (server ? server.price : 0);
    }, 0);

    document.getElementById('rented-servers').textContent = rentedCount;
    document.getElementById('total-price').textContent = `€${totalPrice.toFixed(2)}`;

    // Calculate and display expiry time based on balance and total price
    updateExpiryTime(totalPrice);
}

function updateExpiryTime(totalPrice) {
    const balanceEl = document.getElementById('balance');
    const expiryEl = document.getElementById('expiry-time');

    // Parse balance from the element (remove '€' prefix if present)
    const balanceText = balanceEl.textContent.replace('€', '').trim();
    const balance = parseFloat(balanceText);

    // If no rentals, show "--"
    if (totalPrice === 0 || isNaN(balance)) {
        expiryEl.textContent = '--';
        return;
    }

    // Calculate hourly cost: total_price / 29 days / 24 hours
    const hourlyCost = totalPrice / 29 / 24;

    // Calculate hours until balance is drained
    const hoursLeft = balance / hourlyCost;

    // Convert to days and hours
    const days = Math.floor(hoursLeft / 24);
    const hours = Math.floor(hoursLeft % 24);

    // Format the display
    if (days > 0) {
        expiryEl.textContent = `${days}d ${hours}h`;
    } else if (hours > 0) {
        expiryEl.textContent = `${hours}h`;
    } else {
        // Less than 1 hour - show in minutes
        const minutes = Math.floor((hoursLeft * 60) % 60);
        expiryEl.textContent = minutes > 0 ? `${minutes}m` : '< 1h';
    }

    // Add warning class if balance is low (less than 24 hours)
    if (hoursLeft < 24) {
        expiryEl.style.color = 'var(--danger)';
    } else {
        expiryEl.style.color = '';
    }
}

function showCredentialsModal(newRentals) {
    const credentialsList = document.getElementById('credentials-list');
    const modal = document.getElementById('credentials-modal');

    credentialsList.innerHTML = newRentals.map((rental, index) => {
        const server = servers.find(s => s.id === rental.server_id);
        const flag = server ? getFlag(server.countryCode) : '🌐';
        const hostname = `${rental.server_codename}.socks5.website`;
        const fullUrl = `socks5://${rental.username}:${rental.password}@${hostname}:${rental.port}`;

        // Escape values for HTML attributes
        const escUsername = escapeHtml(rental.username);
        const escPassword = escapeHtml(rental.password);
        const escHostname = escapeHtml(hostname);
        const escPort = rental.port;
        const escFullUrl = escapeHtml(fullUrl);

        return `
        <div class="credential-item" data-index="${index}">
            <div class="credential-header">
                <span class="credential-country">${flag} ${escapeHtml(rental.server_codename)}</span>
            </div>
            <div class="credential-row">
                <span class="credential-label">Username</span>
                <span class="credential-value" data-value="${escUsername}" title="${escUsername}">${escUsername}</span>
                <button class="btn-copy-cred" data-copy="username">Copy</button>
            </div>
            <div class="credential-row">
                <span class="credential-label">Password</span>
                <span class="credential-value" data-value="${escPassword}" title="${escPassword}">${escPassword}</span>
                <button class="btn-copy-cred" data-copy="password">Copy</button>
            </div>
            <div class="credential-row">
                <span class="credential-label">Host:Port</span>
                <span class="credential-value" data-value="${escHostname}:${escPort}" title="${escHostname}:${escPort}">${escHostname}:${escPort}</span>
                <button class="btn-copy-cred" data-copy="hostport">Copy</button>
            </div>
            <div class="credential-row">
                <span class="credential-label">Full URL</span>
                <span class="credential-value full-url" data-value="${escFullUrl}" title="${escFullUrl}">${escFullUrl}</span>
                <button class="btn-copy-cred" data-copy="fullurl">Copy</button>
            </div>
        </div>
    `;
    }).join('');

    // Add event listeners to copy buttons
    credentialsList.querySelectorAll('.btn-copy-cred').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const item = e.target.closest('.credential-item');
            const copyType = e.target.dataset.copy;
            let value;

            if (copyType === 'username') {
                value = item.querySelector('[data-copy="username"]').previousElementSibling.dataset.value;
            } else if (copyType === 'password') {
                value = item.querySelector('[data-copy="password"]').previousElementSibling.dataset.value;
            } else if (copyType === 'hostport') {
                value = item.querySelector('[data-copy="hostport"]').previousElementSibling.dataset.value;
            } else if (copyType === 'fullurl') {
                value = item.querySelector('[data-copy="fullurl"]').previousElementSibling.dataset.value;
            }

            copySingleCredential(e.target, value);
        });
    });

    modal.classList.add('active');
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function copySingleCredential(btn, text) {
    if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(text).then(() => {
            showCopyFeedbackForButton(btn);
        }).catch(() => {
            fallbackCopySingle(btn, text);
        });
    } else {
        fallbackCopySingle(btn, text);
    }
}

function fallbackCopySingle(btn, text) {
    const textArea = document.createElement('textarea');
    textArea.value = text;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    textArea.style.top = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();

    try {
        document.execCommand('copy');
        showCopyFeedbackForButton(btn);
    } catch (err) {
        prompt('Copy this text:', text);
    }

    document.body.removeChild(textArea);
}

function showCopyFeedbackForButton(btn) {
    const originalText = btn.textContent;
    btn.textContent = 'Copied!';
    btn.classList.add('copied');
    setTimeout(() => {
        btn.textContent = originalText;
        btn.classList.remove('copied');
    }, 2000);
}

function copyCredentials(username, password, hostname, port) {
    const creds = `socks5://${username}:${password}@${hostname}:${port}`;

    if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(creds).then(() => {
            showCopyFeedback();
        }).catch(() => {
            fallbackCopy(creds);
        });
    } else {
        fallbackCopy(creds);
    }
}

function fallbackCopy(text) {
    const textArea = document.createElement('textarea');
    textArea.value = text;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    textArea.style.top = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();

    try {
        document.execCommand('copy');
        showCopyFeedback();
    } catch (err) {
        prompt('Copy these credentials:', text);
    }

    document.body.removeChild(textArea);
}

function showCopyFeedback() {
    const feedback = document.createElement('div');
    feedback.textContent = 'Copied!';
    feedback.style.cssText = 'position:fixed;top:20px;left:50%;transform:translateX(-50%);background:#10b981;color:white;padding:8px 16px;border-radius:6px;font-size:14px;z-index:1000;';
    document.body.appendChild(feedback);
    setTimeout(() => feedback.remove(), 2000);
}

function toggleSort(column) {
    const th = document.querySelector(`th[data-sort="${column}"]`);

    document.querySelectorAll('th.sortable').forEach(t => {
        t.classList.remove('sort-asc', 'sort-desc');
    });

    if (sortColumn === column) {
        sortDirection = sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
        sortColumn = column;
        sortDirection = th.dataset.default === 'asc' ? 'asc' : 'desc';
    }

    th.classList.add(`sort-${sortDirection}`);
    renderTable();
}

document.querySelectorAll('th.sortable').forEach(th => {
    th.addEventListener('click', () => toggleSort(th.dataset.sort));
});

updateDashboard();

const defaultSortTh = document.querySelector('th[data-sort="price"]');
if (defaultSortTh) {
    defaultSortTh.classList.add('sort-asc');
}

document.querySelectorAll('.help-btn-small').forEach(btn => {
    btn.addEventListener('click', (e) => showCredentialsHelp(e));
});

document.getElementById('close-modal').addEventListener('click', () => {
    document.getElementById('help-modal').classList.remove('active');
});

document.getElementById('help-modal').addEventListener('click', (e) => {
    if (e.target.id === 'help-modal') {
        document.getElementById('help-modal').classList.remove('active');
    }
});

document.getElementById('rent-modal').addEventListener('click', (e) => {
    if (e.target.id === 'rent-modal') {
        closeRentModal();
    }
});

document.getElementById('credentials-modal').addEventListener('click', (e) => {
    if (e.target.id === 'credentials-modal') {
        document.getElementById('credentials-modal').classList.remove('active');
    }
});

document.getElementById('slots-slider').addEventListener('input', (e) => {
    const slots = e.target.value;
    document.getElementById('slots-value').textContent = slots;
    updateRentModalPrice(currentRentingServerId, slots);
});

// Reset slider visibility when modal is closed
function closeRentModal() {
    const sliderContainer = document.querySelector('.slider-container');
    sliderContainer.style.display = 'block';
    document.getElementById('rent-modal').classList.remove('active');
}

document.getElementById('close-rent-modal').addEventListener('click', closeRentModal);

document.getElementById('close-credentials-modal').addEventListener('click', () => {
    document.getElementById('credentials-modal').classList.remove('active');
});

document.getElementById('confirm-rent-btn').addEventListener('click', confirmRent);

document.getElementById('proxies-help-btn').addEventListener('click', (e) => showCredentialsHelp(e));

document.querySelectorAll('.help-btn-small').forEach(btn => {
    btn.addEventListener('click', (e) => showCredentialsHelp(e));
});

function updateTabCounters() {
    const availableCount = servers.filter(s => s.availableSlots > 0).length;
    const rentedCount = rentals.length;

    document.getElementById('available-counter').textContent = `(${availableCount})`;
    document.getElementById('proxies-counter').textContent = `(${rentedCount})`;
}

function switchTab(tabName) {
    document.querySelectorAll('.tab-button').forEach(btn => {
        btn.classList.remove('active');
    });
    document.querySelectorAll('.tab-pane').forEach(pane => {
        pane.classList.remove('active');
    });

    document.querySelector(`[data-tab="${tabName}"]`).classList.add('active');
    document.getElementById(`${tabName}-tab`).classList.add('active');
}

document.querySelectorAll('.tab-button').forEach(btn => {
    btn.addEventListener('click', () => {
        switchTab(btn.dataset.tab);
    });
});

fetchServers();

// Account Settings Modal
const settingsBtn = document.getElementById('settings-btn');
const accountSettingsModal = document.getElementById('account-settings-modal');
const closeSettingsModal = document.getElementById('close-settings-modal');
const secretKeyDisplay = document.getElementById('secret-key-display');
const copySkBtn = document.getElementById('copy-sk-btn');
const copyFeedback = document.getElementById('copy-feedback');
const loginSkInput = document.getElementById('login-sk-input');
const loginSkBtn = document.getElementById('login-sk-btn');
const loginError = document.getElementById('login-error');
const logoutBtn = document.getElementById('logout-btn');

// Open account settings modal and fetch secret key
if (settingsBtn) {
    settingsBtn.addEventListener('click', () => {
        accountSettingsModal.classList.add('active');
        fetchSecretKey();
    });
}

// Close account settings modal
if (closeSettingsModal) {
    closeSettingsModal.addEventListener('click', () => {
        accountSettingsModal.classList.remove('active');
        clearLoginError();
    });
}

// Close modal on overlay click
if (accountSettingsModal) {
    accountSettingsModal.addEventListener('click', (e) => {
        if (e.target.id === 'account-settings-modal') {
            accountSettingsModal.classList.remove('active');
            clearLoginError();
        }
    });
}

// Fetch secret key from backend
async function fetchSecretKey() {
    secretKeyDisplay.textContent = 'Loading...';
    copyFeedback.textContent = '';
    
    try {
        const response = await fetch('/api/auth/secret-key');
        const data = await response.json();
        
        if (data.success && data.secret_key) {
            secretKeyDisplay.textContent = data.secret_key;
        } else {
            secretKeyDisplay.textContent = 'Not available';
        }
    } catch (error) {
        console.error('Failed to fetch secret key:', error);
        secretKeyDisplay.textContent = 'Error loading';
    }
}

// Copy secret key to clipboard with mobile fallback
function copySecretKey() {
    const secretKey = secretKeyDisplay.textContent;
    
    if (!secretKey || secretKey === 'Loading...' || secretKey === 'Not available' || secretKey === 'Error loading') {
        return;
    }
    
    if (navigator.clipboard && window.isSecureContext) {
        navigator.clipboard.writeText(secretKey).then(() => {
            showCopyFeedbackMessage('Copied!');
        }).catch(() => {
            fallbackCopySecretKey(secretKey);
        });
    } else {
        fallbackCopySecretKey(secretKey);
    }
}

function fallbackCopySecretKey(text) {
    const textArea = document.createElement('textarea');
    textArea.value = text;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    textArea.style.top = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    
    try {
        document.execCommand('copy');
        showCopyFeedbackMessage('Copied!');
    } catch (err) {
        // Final fallback: select the text in the code element
        const range = document.createRange();
        range.selectNode(secretKeyDisplay);
        window.getSelection().removeAllRanges();
        window.getSelection().addRange(range);
        showCopyFeedbackMessage('Selected! Tap to copy');
    }
    
    document.body.removeChild(textArea);
}

function showCopyFeedbackMessage(message) {
    copyFeedback.textContent = message;
    setTimeout(() => {
        copyFeedback.textContent = '';
    }, 2000);
}

// Copy button click handler
if (copySkBtn) {
    copySkBtn.addEventListener('click', copySecretKey);
}

// Login with secret key
async function loginWithSecretKey() {
    const sk = loginSkInput.value.trim();
    
    if (!sk) {
        showLoginError('Please enter your secret key');
        return;
    }
    
    if (!sk.startsWith('SK_')) {
        showLoginError('Secret key must start with SK_');
        return;
    }
    
    loginSkBtn.disabled = true;
    loginSkBtn.innerHTML = '<span class="btn-spinner"></span>';
    clearLoginError();
    
    try {
        const response = await fetch('/api/auth/login', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ secret_key: sk }),
            credentials: 'include',
        });
        
        const data = await response.json();
        
        if (data.success) {
            // Login successful, reload the page to refresh the session
            window.location.reload();
        } else {
            showLoginError(data.error || 'Login failed');
        }
    } catch (error) {
        console.error('Login error:', error);
        showLoginError('Failed to connect to server');
    } finally {
        loginSkBtn.disabled = false;
        loginSkBtn.textContent = 'Login';
    }
}

function showLoginError(message) {
    loginError.textContent = message;
}

function clearLoginError() {
    loginError.textContent = '';
}

// Login button click handler
if (loginSkBtn) {
    loginSkBtn.addEventListener('click', loginWithSecretKey);
}

// Allow pressing Enter in the login input
if (loginSkInput) {
    loginSkInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            loginWithSecretKey();
        }
    });
}

// Logout handler
async function logout() {
    if (!confirm('Are you sure you want to logout? This will clear your current session and generate a new secret key. Your current secret key will no longer be accessible from this device.')) {
        return;
    }
    
    logoutBtn.disabled = true;
    logoutBtn.innerHTML = '<span class="btn-spinner"></span>';
    
    try {
        const response = await fetch('/api/auth/logout', {
            method: 'POST',
            credentials: 'include',
        });
        
        if (response.ok || response.status === 302) {
            // Redirect to transit endpoint which will generate a new secret key
            window.location.href = '/';
        } else {
            alert('Logout failed. Please try again.');
            logoutBtn.disabled = false;
            logoutBtn.textContent = 'Logout & Generate New Key';
        }
    } catch (error) {
        console.error('Logout error:', error);
        alert('Logout failed. Please try again.');
        logoutBtn.disabled = false;
        logoutBtn.textContent = 'Logout & Generate New Key';
    }
}

// Logout button click handler
if (logoutBtn) {
    logoutBtn.addEventListener('click', logout);
}

// Support chat button handler
const supportBtn = document.getElementById('support-btn');
if (supportBtn) {
    supportBtn.addEventListener('click', () => {
        window.location.href = '/support';
    });
}

// Contact modal handlers
const contactFooterLink = document.getElementById('contact-footer-link');
const contactModal = document.getElementById('contact-modal');
const closeContactModal = document.getElementById('close-contact-modal');
const contactChatBtn = document.getElementById('contact-chat-btn');

// Open contact modal from footer link
if (contactFooterLink) {
    contactFooterLink.addEventListener('click', (e) => {
        e.preventDefault();
        contactModal.classList.add('active');
    });
}

// Close contact modal
if (closeContactModal) {
    closeContactModal.addEventListener('click', () => {
        contactModal.classList.remove('active');
    });
}

// Close modal on overlay click
if (contactModal) {
    contactModal.addEventListener('click', (e) => {
        if (e.target.id === 'contact-modal') {
            contactModal.classList.remove('active');
        }
    });
}

// Open support chat from contact modal
if (contactChatBtn) {
    contactChatBtn.addEventListener('click', () => {
        window.location.href = '/support';
    });
}

// Top Up Balance modal handlers
const topupBtn = document.getElementById('topup-btn');
const topupModal = document.getElementById('topup-modal');
const closeTopupModal = document.getElementById('close-topup-modal');

// Open topup modal
if (topupBtn) {
    topupBtn.addEventListener('click', () => {
        topupModal.classList.add('active');
    });
}

// Close topup modal
if (closeTopupModal) {
    closeTopupModal.addEventListener('click', () => {
        topupModal.classList.remove('active');
    });
}

// Close modal on overlay click
if (topupModal) {
    topupModal.addEventListener('click', (e) => {
        if (e.target.id === 'topup-modal') {
            topupModal.classList.remove('active');
        }
    });
}

// Handle payment method selection
document.querySelectorAll('.select-payment-btn').forEach(btn => {
    btn.addEventListener('click', () => {
        const paymentMethod = btn.dataset.payment;
        handlePaymentSelection(paymentMethod);
    });
});

function handlePaymentSelection(method) {
    topupModal.classList.remove('active');
    
    switch (method) {
        case 'monero':
            window.open('/payment/process?provider=cryptowrap', '_blank');
            // add coin specification later, then cryptowrap will be processing something except monero, but by default it's monero
            break;
        case 'trocador':
            window.open('/payment/process?provider=trocador', '_blank');
            break;
        case 'platega':
            openPlategaPaymentModal();
            break;
    }
}

function showPaymentInstructions(method) {
    if (method === 'monero') {
        alert('🚧🚧🚧');
    }
}

// Coupon Redemption Handler
const redeemCouponBtn = document.getElementById('redeem-coupon-btn');
const couponCodeInput = document.getElementById('coupon-code-input');
const couponMessage = document.getElementById('coupon-message');

if (redeemCouponBtn && couponCodeInput && couponMessage) {
    redeemCouponBtn.addEventListener('click', handleCouponRedemption);
    
    // Also allow Enter key to trigger redemption
    couponCodeInput.addEventListener('keypress', (e) => {
        if (e.key === 'Enter') {
            handleCouponRedemption();
        }
    });
}

async function handleCouponRedemption() {
    const code = couponCodeInput.value.trim();
    
    if (!code) {
        showCouponMessage('Please enter a coupon code', 'error');
        return;
    }
    
    // Disable button during request
    redeemCouponBtn.disabled = true;
    redeemCouponBtn.textContent = 'Redeeming...';
    
    try {
        const response = await fetch('/api/coupon/redeem', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ code: code }),
        });
        
        const data = await response.json();
        
        if (data.success) {
            showCouponMessage(data.message, 'success');
            couponCodeInput.value = '';
            
            // Update balance display after a short delay
            setTimeout(() => {
                location.reload(); // Reload to show new balance
            }, 1500);
        } else {
            showCouponMessage(data.message, 'error');
        }
    } catch (error) {
        console.error('Coupon redemption error:', error);
        showCouponMessage('Failed to redeem coupon. Please try again.', 'error');
    } finally {
        redeemCouponBtn.disabled = false;
        redeemCouponBtn.textContent = 'Redeem';
    }
}

function showCouponMessage(message, type) {
    couponMessage.textContent = message;
    couponMessage.className = 'coupon-message ' + type;
    
    // Auto-hide success messages after 5 seconds
    if (type === 'success') {
        setTimeout(() => {
            couponMessage.className = 'coupon-message';
        }, 5000);
    }
}


// Table scroll indicators
function updateTableScrollIndicators() {
    const tableContainer = document.querySelector('.table-container');
    const table = document.querySelector('.servers-table');
    if (!tableContainer || !table) return;

    const scrollLeft = tableContainer.scrollLeft;
    const scrollWidth = tableContainer.scrollWidth;
    const clientWidth = tableContainer.clientWidth;

    tableContainer.classList.toggle('scroll-left', scrollLeft > 0);
    tableContainer.classList.toggle('scroll-right', scrollLeft < scrollWidth - clientWidth - 1);
}

// Initialize scroll indicators
const tableContainer = document.querySelector('.table-container');
if (tableContainer) {
    tableContainer.addEventListener('scroll', updateTableScrollIndicators);
    window.addEventListener('resize', updateTableScrollIndicators);
    // Initial check after a short delay to ensure table is rendered
    setTimeout(updateTableScrollIndicators, 100);
}
