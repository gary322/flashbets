import React, { useEffect, useState } from 'react';
import { Card, Table, Spin, Typography, Space, Tag, Progress } from 'antd';
import polymarketService, { OrderBook as OrderBookType, OrderBookLevel } from '../../services/polymarketService';

const { Text, Title } = Typography;

interface OrderBookProps {
  tokenId: string;
  refreshInterval?: number;
}

const OrderBook: React.FC<OrderBookProps> = ({ tokenId, refreshInterval = 5000 }) => {
  const [orderBook, setOrderBook] = useState<OrderBookType | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchOrderBook = async () => {
      try {
        const data = await polymarketService.getOrderBook(tokenId);
        setOrderBook(data);
      } catch (error) {
        console.error('Failed to fetch order book:', error);
      } finally {
        setLoading(false);
      }
    };

    fetchOrderBook();
    const interval = setInterval(fetchOrderBook, refreshInterval);
    return () => clearInterval(interval);
  }, [tokenId, refreshInterval]);

  const renderOrderLevel = (levels: OrderBookLevel[], type: 'bid' | 'ask') => {
    const maxSize = Math.max(...levels.map(l => parseFloat(l.size)));
    
    return levels.slice(0, 10).map((level, index) => (
      <div key={index} className="order-level">
        <Space style={{ width: '100%', justifyContent: 'space-between' }}>
          <Text type={type === 'bid' ? 'success' : 'danger'}>
            ${parseFloat(level.price).toFixed(4)}
          </Text>
          <Text>{parseFloat(level.size).toFixed(2)}</Text>
          <Text type="secondary">{level.numOrders}</Text>
          <Progress
            percent={(parseFloat(level.size) / maxSize) * 100}
            showInfo={false}
            strokeColor={type === 'bid' ? '#52c41a' : '#ff4d4f'}
            style={{ width: 100 }}
          />
        </Space>
      </div>
    ));
  };

  if (loading) {
    return (
      <Card>
        <Spin size="large" />
      </Card>
    );
  }

  if (!orderBook) {
    return (
      <Card>
        <Text>No order book data available</Text>
      </Card>
    );
  }

  return (
    <Card title="Order Book" extra={
      <Space>
        {orderBook.spread && (
          <Tag color="blue">Spread: ${orderBook.spread}</Tag>
        )}
        {orderBook.midPrice && (
          <Tag color="green">Mid: ${orderBook.midPrice}</Tag>
        )}
      </Space>
    }>
      <div style={{ display: 'flex', gap: 20 }}>
        <div style={{ flex: 1 }}>
          <Title level={5} type="success">Bids</Title>
          <Space direction="vertical" style={{ width: '100%' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between' }}>
              <Text type="secondary">Price</Text>
              <Text type="secondary">Size</Text>
              <Text type="secondary">Orders</Text>
              <Text type="secondary">Depth</Text>
            </div>
            {renderOrderLevel(orderBook.bids, 'bid')}
          </Space>
        </div>
        
        <div style={{ flex: 1 }}>
          <Title level={5} type="danger">Asks</Title>
          <Space direction="vertical" style={{ width: '100%' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between' }}>
              <Text type="secondary">Price</Text>
              <Text type="secondary">Size</Text>
              <Text type="secondary">Orders</Text>
              <Text type="secondary">Depth</Text>
            </div>
            {renderOrderLevel(orderBook.asks, 'ask')}
          </Space>
        </div>
      </div>
    </Card>
  );
};

export default OrderBook;