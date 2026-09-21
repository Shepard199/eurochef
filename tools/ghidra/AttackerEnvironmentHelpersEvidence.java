import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class AttackerEnvironmentHelpersEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionContaining(a);if(f==null){f=getFunctionAt(a);if(f==null)f=createFunction(a,name);}
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,120,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  long[] scalars={0x005DDE94L,0x005E2338L,0x005DFC48L,0x005DD384L};
  for(long x:scalars){int v=getInt(toAddr(x));println(String.format("SCALAR 0x%08X bits=0x%08X float=%.9f",x,Integer.toUnsignedLong(v),Float.intBitsToFloat(v)));}
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x00456E20L,"tmp_456e20",d);dc(0x00456E40L,"tmp_456e40",d);dc(0x00456920L,"tmp_456920",d);dc(0x00454DE0L,"tmp_454de0",d);
  d.dispose();
 }
}