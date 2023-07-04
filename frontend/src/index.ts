import { BrowserProvider } from 'ethers';

// If MetaMask is installed there will be an `ethereum` object on the `window`
const provider = new BrowserProvider((window as any).ethereum);

// Get the Colony's XDAI funding in the ROOT pot (id 1)
const start = async () => {
  // This will try to connect the page to MetaMask
  await provider.send('eth_requestAccounts', []);

  const signer = await provider.getSigner();
  const address = await signer.getAddress();

  const pathSplit = window.location.pathname.split('/');
  const username = pathSplit[2]
  const sessionId = pathSplit[3]
  const signature = await signer.signMessage(`Please sign this message to connect your Discord username ${username} with your wallet address. Session ID: ${sessionId}`);

  const response = await fetch(window.location.href, {
    method: 'POST',
    headers: {
      'Accept': 'application/json',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      signature,
      address,
    })
  });

  if (response.ok) {
    alert('Successfully connected!');
  }
};

// start();
