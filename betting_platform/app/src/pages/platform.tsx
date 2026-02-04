import React, { useEffect } from 'react';
import Head from 'next/head';

export default function Platform() {
  useEffect(() => {
    // Redirect to the static platform UI
    window.location.href = '/platform_ui.html';
  }, []);

  return (
    <>
      <Head>
        <title>Quantum Betting Platform</title>
        <meta name="description" content="Native Solana betting platform with quantum features" />
      </Head>
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        backgroundColor: '#000',
        color: '#fff'
      }}>
        <p>Redirecting to platform...</p>
      </div>
    </>
  );
}