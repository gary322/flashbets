import React, { useState, useEffect } from 'react';
import {
  Card,
  Form,
  Input,
  Button,
  Select,
  Radio,
  Space,
  Typography,
  Slider,
  Row,
  Col,
  Alert,
  Spin,
  message,
  Statistic,
} from 'antd';
import {
  ArrowUpOutlined,
  ArrowDownOutlined,
  SwapOutlined,
  WalletOutlined,
} from '@ant-design/icons';
import polymarketService, {
  CreateOrderParams,
  Balance,
  MarketData,
} from '../../services/polymarketService';
import { ethers } from 'ethers';

const { Title, Text } = Typography;
const { Option } = Select;

interface TradingPanelProps {
  marketId: string;
  conditionId: string;
  tokenId: string;
  outcome: number;
  onOrderPlaced?: () => void;
}

const TradingPanel: React.FC<TradingPanelProps> = ({
  marketId,
  conditionId,
  tokenId,
  outcome,
  onOrderPlaced,
}) => {
  const [form] = Form.useForm();
  const [loading, setLoading] = useState(false);
  const [side, setSide] = useState<'buy' | 'sell'>('buy');
  const [orderType, setOrderType] = useState<'gtc' | 'fok' | 'ioc'>('gtc');
  const [balance, setBalance] = useState<Balance | null>(null);
  const [marketData, setMarketData] = useState<MarketData | null>(null);
  const [estimatedCost, setEstimatedCost] = useState<string>('0');
  const [estimatedReturn, setEstimatedReturn] = useState<string>('0');

  useEffect(() => {
    fetchBalance();
    fetchMarketData();
  }, []);

  useEffect(() => {
    const values = form.getFieldsValue(['size', 'price']);
    calculateEstimates(values.size, values.price);
  }, [form, side]);

  const fetchBalance = async () => {
    try {
      const balanceData = await polymarketService.getBalances();
      setBalance(balanceData);
    } catch (error) {
      console.error('Failed to fetch balance:', error);
    }
  };

  const fetchMarketData = async () => {
    try {
      const data = await polymarketService.getMarketData(conditionId);
      setMarketData(data);
    } catch (error) {
      console.error('Failed to fetch market data:', error);
    }
  };

  const calculateEstimates = (size: string, price: string) => {
    if (!size || !price) {
      setEstimatedCost('0');
      setEstimatedReturn('0');
      return;
    }

    const sizeNum = parseFloat(size);
    const priceNum = parseFloat(price);

    if (side === 'buy') {
      const cost = sizeNum * priceNum;
      const returnAmount = sizeNum;
      setEstimatedCost(cost.toFixed(2));
      setEstimatedReturn(returnAmount.toFixed(2));
    } else {
      const cost = sizeNum;
      const returnAmount = sizeNum * priceNum;
      setEstimatedCost(cost.toFixed(2));
      setEstimatedReturn(returnAmount.toFixed(2));
    }
  };

  const handleSubmit = async (values: any) => {
    setLoading(true);
    try {
      const orderParams: CreateOrderParams = {
        marketId,
        conditionId,
        tokenId,
        outcome,
        side,
        size: values.size,
        price: values.price,
        orderType,
      };

      const result = await polymarketService.placeOrder(orderParams);
      message.success(`Order placed successfully! ID: ${result.orderId}`);
      
      form.resetFields();
      fetchBalance();
      if (onOrderPlaced) onOrderPlaced();
    } catch (error: any) {
      message.error(error.message || 'Failed to place order');
      console.error('Order placement error:', error);
    } finally {
      setLoading(false);
    }
  };

  const setQuickAmount = (percentage: number) => {
    if (!balance) return;
    
    const available = parseFloat(balance.availableBalance);
    const amount = (available * percentage / 100).toFixed(2);
    form.setFieldsValue({ size: amount });
    
    const price = form.getFieldValue('price');
    calculateEstimates(amount, price);
  };

  const setMarketPrice = () => {
    if (!marketData) return;
    
    const price = side === 'buy' 
      ? marketData.ask || marketData.lastPrice
      : marketData.bid || marketData.lastPrice;
    
    if (price) {
      form.setFieldsValue({ price });
      const size = form.getFieldValue('size');
      calculateEstimates(size, price);
    }
  };

  return (
    <Card title="Place Order" className="trading-panel">
      <Space direction="vertical" style={{ width: '100%' }} size="large">
        {/* Balance Display */}
        {balance && (
          <Card size="small" style={{ background: '#fafafa' }}>
            <Row gutter={16}>
              <Col span={12}>
                <Statistic
                  title="Available USDC"
                  value={parseFloat(balance.availableBalance).toFixed(2)}
                  prefix={<WalletOutlined />}
                  suffix="USDC"
                />
              </Col>
              <Col span={12}>
                <Statistic
                  title="Locked in Orders"
                  value={parseFloat(balance.lockedInOrders).toFixed(2)}
                  suffix="USDC"
                />
              </Col>
            </Row>
          </Card>
        )}

        {/* Order Side Selection */}
        <Radio.Group
          value={side}
          onChange={(e) => setSide(e.target.value)}
          buttonStyle="solid"
          style={{ width: '100%' }}
        >
          <Radio.Button value="buy" style={{ width: '50%', textAlign: 'center' }}>
            <ArrowUpOutlined /> Buy (Yes)
          </Radio.Button>
          <Radio.Button value="sell" style={{ width: '50%', textAlign: 'center' }}>
            <ArrowDownOutlined /> Sell (No)
          </Radio.Button>
        </Radio.Group>

        {/* Order Form */}
        <Form
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          onValuesChange={(_, values) => calculateEstimates(values.size, values.price)}
        >
          {/* Price Input */}
          <Form.Item
            name="price"
            label="Price"
            rules={[
              { required: true, message: 'Please enter price' },
              { 
                validator: (_, value) => {
                  const price = parseFloat(value);
                  if (price <= 0 || price >= 1) {
                    return Promise.reject('Price must be between 0 and 1');
                  }
                  return Promise.resolve();
                }
              }
            ]}
          >
            <Input
              prefix="$"
              placeholder="0.50"
              suffix={
                <Button type="link" size="small" onClick={setMarketPrice}>
                  Market
                </Button>
              }
            />
          </Form.Item>

          {/* Size Input */}
          <Form.Item
            name="size"
            label="Size"
            rules={[
              { required: true, message: 'Please enter size' },
              { 
                validator: (_, value) => {
                  if (parseFloat(value) <= 0) {
                    return Promise.reject('Size must be greater than 0');
                  }
                  return Promise.resolve();
                }
              }
            ]}
          >
            <Input
              placeholder="100"
              suffix="Shares"
            />
          </Form.Item>

          {/* Quick Amount Buttons */}
          <Space style={{ marginBottom: 16 }}>
            <Button size="small" onClick={() => setQuickAmount(25)}>25%</Button>
            <Button size="small" onClick={() => setQuickAmount(50)}>50%</Button>
            <Button size="small" onClick={() => setQuickAmount(75)}>75%</Button>
            <Button size="small" onClick={() => setQuickAmount(100)}>Max</Button>
          </Space>

          {/* Order Type */}
          <Form.Item label="Order Type">
            <Select value={orderType} onChange={setOrderType}>
              <Option value="gtc">Good Till Cancelled (GTC)</Option>
              <Option value="fok">Fill or Kill (FOK)</Option>
              <Option value="ioc">Immediate or Cancel (IOC)</Option>
            </Select>
          </Form.Item>

          {/* Cost Estimates */}
          <Card size="small" style={{ marginBottom: 16 }}>
            <Space direction="vertical" style={{ width: '100%' }}>
              <Row justify="space-between">
                <Text type="secondary">Estimated Cost:</Text>
                <Text strong>${estimatedCost} USDC</Text>
              </Row>
              <Row justify="space-between">
                <Text type="secondary">Potential Return:</Text>
                <Text strong type="success">${estimatedReturn} USDC</Text>
              </Row>
              <Row justify="space-between">
                <Text type="secondary">Est. Fees:</Text>
                <Text>${(parseFloat(estimatedCost) * 0.001).toFixed(4)} USDC</Text>
              </Row>
            </Space>
          </Card>

          {/* Submit Button */}
          <Form.Item>
            <Button
              type="primary"
              htmlType="submit"
              loading={loading}
              block
              size="large"
              style={{
                background: side === 'buy' ? '#52c41a' : '#ff4d4f',
                borderColor: side === 'buy' ? '#52c41a' : '#ff4d4f',
              }}
            >
              {loading ? 'Placing Order...' : `Place ${side.toUpperCase()} Order`}
            </Button>
          </Form.Item>
        </Form>

        {/* Market Info */}
        {marketData && (
          <Alert
            message="Market Info"
            description={
              <Space direction="vertical">
                <Text>Last Price: ${marketData.lastPrice || 'N/A'}</Text>
                <Text>24h Volume: ${parseFloat(marketData.volume24h).toLocaleString()}</Text>
                <Text>Liquidity: ${parseFloat(marketData.liquidity).toLocaleString()}</Text>
              </Space>
            }
            type="info"
            showIcon
          />
        )}
      </Space>
    </Card>
  );
};

export default TradingPanel;