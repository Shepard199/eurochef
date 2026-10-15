import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.data.*;
public class InputActionNames2C2DEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void dec(long x)throws Exception{
  Function f=getFunctionContaining(toAddr(x)); println(String.format("\n===0x%08X===",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  dec(0x004f9032L);
  dec(0x0043a280L);
  DataIterator it=currentProgram.getListing().getDefinedData(true);
  while(it.hasNext()){
   Data x=it.next();
   if(!x.hasStringValue())continue;
   Object v=x.getValue();
   String s=v==null?"":v.toString().toUpperCase();
   if(s.contains("UP")||s.contains("DOWN")||s.contains("SELECT")||s.contains("BACK")||s.contains("LEFT")||s.contains("RIGHT"))
    println("STR "+x.getAddress()+" = "+v);
  }
  d.dispose();
 }
}