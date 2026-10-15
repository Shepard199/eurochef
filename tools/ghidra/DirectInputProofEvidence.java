import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.util.*;
public class DirectInputProofEvidence extends GhidraScript {
 DecompInterface d=new DecompInterface();
 void createDec(long x)throws Exception{
  Address a=toAddr(x); disassemble(a); Function f=getFunctionAt(a);
  if(f==null){try{f=createFunction(a,null);}catch(Exception e){}}
  println(String.format("\n=== DEC 0x%08X ===",x));
  if(f==null){println("NO_FUNCTION");return;}
  println(f.getName()+" @ "+f.getEntryPoint());
  DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 public void run()throws Exception{
  d.openProgram(currentProgram);
  println("=== SYMBOL SEARCH ===");
  SymbolIterator it=currentProgram.getSymbolTable().getAllSymbols(true);
  String[] needles={"directinput","getdevicestate","acquire","createdevice","keyboard","dinput","getasynckeystate","getkeyboardstate"};
  while(it.hasNext()){
    Symbol s=it.next(); String n=s.getName().toLowerCase();
    for(String q:needles) if(n.contains(q)){println(s.getAddress()+" "+s.getName()+" "+s.getSymbolType());break;}
  }
  long[] a={0x00534ee4L,0x00534ee9L,0x00434c50L,0x004438b0L,0x005352d2L,0x00434560L,0x004345b0L,0x00434730L,0x00434780L,0x00434850L,0x004348d0L,0x00434900L,0x00434940L,0x00434970L,0x00434af0L,0x00434bb0L,0x00534de8L};
  for(long x:a)createDec(x);
  d.dispose();
 }
}