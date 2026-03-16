/**
 * SSE 调试工具
 * 用于诊断 SSE 连接问题
 */

export async function testSseConnection(baseUrl: string, authToken: string): Promise<{
  success: boolean;
  message: string;
  details?: any;
}> {
  try {
    console.log('🔍 Testing SSE connection...');
    console.log(`   Base URL: ${baseUrl}`);
    console.log(`   Auth Token: ${authToken.substring(0, 8)}...`);
    
    const url = `${baseUrl}/api/chat/events?token=${encodeURIComponent(authToken)}`;
    console.log(`   SSE URL: ${url}`);
    
    // 测试 1: 检查 URL 是否可访问
    console.log('\n📡 Test 1: Checking if URL is accessible...');
    try {
      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Accept': 'text/event-stream',
        },
      });
      
      console.log(`   Status: ${response.status}`);
      console.log(`   Content-Type: ${response.headers.get('content-type')}`);
      console.log(`   Transfer-Encoding: ${response.headers.get('transfer-encoding')}`);
      
      if (response.status !== 200) {
        const text = await response.text();
        console.error(`   Error: ${text}`);
        return {
          success: false,
          message: `HTTP ${response.status}: ${text}`,
          details: { status: response.status, text },
        };
      }
      
      if (!response.headers.get('content-type')?.includes('text/event-stream')) {
        console.warn('   ⚠️  Content-Type is not text/event-stream');
      }
    } catch (err) {
      console.error(`   Fetch error: ${err}`);
      return {
        success: false,
        message: `Fetch error: ${err}`,
        details: { error: err },
      };
    }
    
    // 测试 2: 尝试使用 EventSource 连接
    console.log('\n📡 Test 2: Attempting EventSource connection...');
    return new Promise((resolve) => {
      const eventSource = new EventSource(url);
      const timeout = setTimeout(() => {
        eventSource.close();
        resolve({
          success: false,
          message: 'EventSource connection timeout (no open event after 5s)',
        });
      }, 5000);
      
      eventSource.onopen = () => {
        clearTimeout(timeout);
        console.log('   ✅ EventSource opened successfully');
        eventSource.close();
        resolve({
          success: true,
          message: 'SSE connection successful',
        });
      };
      
      eventSource.onerror = (error) => {
        clearTimeout(timeout);
        console.error(`   ❌ EventSource error: ${error}`);
        eventSource.close();
        resolve({
          success: false,
          message: `EventSource error: ${error}`,
          details: { error },
        });
      };
      
      eventSource.onmessage = (event) => {
        console.log(`   📨 Received message: ${event.data.substring(0, 50)}...`);
      };
    });
  } catch (err) {
    console.error(`   Unexpected error: ${err}`);
    return {
      success: false,
      message: `Unexpected error: ${err}`,
      details: { error: err },
    };
  }
}

/**
 * 测试后端连接
 */
export async function testBackendConnection(baseUrl: string, authToken: string): Promise<{
  success: boolean;
  message: string;
  details?: any;
}> {
  try {
    console.log('🔍 Testing backend connection...');
    console.log(`   Base URL: ${baseUrl}`);
    
    const response = await fetch(`${baseUrl}/api/health`, {
      method: 'GET',
      headers: {
        'Authorization': `Bearer ${authToken}`,
      },
    });
    
    console.log(`   Status: ${response.status}`);
    
    if (response.status === 200) {
      const data = await response.json();
      console.log(`   ✅ Backend is healthy`);
      return {
        success: true,
        message: 'Backend connection successful',
        details: data,
      };
    } else {
      const text = await response.text();
      console.error(`   ❌ Backend error: ${text}`);
      return {
        success: false,
        message: `Backend error: ${response.status}`,
        details: { status: response.status, text },
      };
    }
  } catch (err) {
    console.error(`   Connection error: ${err}`);
    return {
      success: false,
      message: `Connection error: ${err}`,
      details: { error: err },
    };
  }
}
